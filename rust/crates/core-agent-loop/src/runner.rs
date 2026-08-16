use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::StreamExt;

use dsh_core_session::event::{SessionEvent, TurnEndReason};
use dsh_core_system_prompt::SystemPrompt;
use dsh_core_tools::Tool;
use dsh_llm_adapter::{
    FunctionCall, FunctionDefinition, LlmAdapter, LlmRequest, Message, ToolCall as LlmToolCall,
    ToolDefinition,
};

use crate::hooks::{
    AgentHooks, PostExecuteCtx, PreExecuteCtx, PreRequestCtx, PreStepCtx,
};
use crate::state::{LoopError, TurnState};

/// Token counting estimate (4 chars ≈ 1 token).
fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

/// Current Unix epoch milliseconds.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Token usage statistics.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// Agent loop runner.
///
/// Drives the turn state machine: ClaimingInput → AssemblingPrompt →
/// Streaming → ExecutingTools → Closed.  Emits full lifecycle events
/// (`turn/start`, `step/start`, `step/end`, `turn/end`) into the session log.
pub struct AgentRunner {
    llm: Arc<dyn LlmAdapter>,
    tools: HashMap<String, Arc<dyn Tool>>,
    session_log: Vec<SessionEvent>,
    state: TurnState,
    model: String,
    system_prompt: SystemPrompt,
    hooks: AgentHooks,
    max_context_tokens: usize,
    max_turns: usize,
    max_iterations: usize,
    token_usage: TokenUsage,

    // Monotonic counters.
    seq: u64,
    current_turn: u64,
    /// Whether the session is still in seed (construction) state.
    is_seed: bool,
}

impl AgentRunner {
    /// Create a new agent runner.
    pub fn new(llm: Arc<dyn LlmAdapter>, model: String) -> Self {
        Self {
            llm,
            tools: HashMap::new(),
            session_log: Vec::new(),
            state: TurnState::ClaimingInput,
            model,
            system_prompt: SystemPrompt::new(),
            hooks: AgentHooks::new(),
            max_context_tokens: 128000,
            max_turns: 10,
            max_iterations: 20,
            token_usage: TokenUsage::default(),
            seq: 0,
            current_turn: 0,
            is_seed: true,
        }
    }

    /// Set the system prompt. Registers the prompt as a single section with order 0.
    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        self.system_prompt.register("default", 0, prompt);
        self
    }

    /// Access the system prompt assembler for section registration.
    pub fn system_prompt(&mut self) -> &mut SystemPrompt {
        &mut self.system_prompt
    }

    /// Access the hook registry for registering extension hooks.
    pub fn hooks(&mut self) -> &mut AgentHooks {
        &mut self.hooks
    }

    /// Whether the session is still in seed (construction) state.
    /// Events logged before `end_seed()` are part of the seed.
    pub fn is_seed(&self) -> bool {
        self.is_seed
    }

    /// End the seed phase. Emits an `EndSeed` event and marks the session as live.
    pub fn end_seed(&mut self) {
        if self.is_seed {
            let seq = self.next_seq();
            let time = now_ms();
            self.session_log.push(SessionEvent::EndSeed { seq, time });
            self.is_seed = false;
        }
    }

    /// Set max context tokens.
    pub fn with_max_context_tokens(mut self, max: usize) -> Self {
        self.max_context_tokens = max;
        self
    }

    /// Set max turns per conversation.
    pub fn with_max_turns(mut self, max: usize) -> Self {
        self.max_turns = max;
        self
    }

    /// Set max iterations per turn (tool call loops).
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Register a tool by name.
    pub fn register_tool(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.schema().name.clone();
        self.tools.insert(name, tool);
    }

    /// Return the current state.
    pub fn state(&self) -> &TurnState {
        &self.state
    }

    /// Return a reference to the session log.
    pub fn session_log(&self) -> &[SessionEvent] {
        &self.session_log
    }

    /// Return token usage statistics.
    pub fn token_usage(&self) -> &TokenUsage {
        &self.token_usage
    }

    /// Increment seq and return the new value.
    fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    /// Build tool definitions for LLM request.
    fn build_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|tool| {
                let schema = tool.schema();
                ToolDefinition {
                    tool_type: "function".into(),
                    function: FunctionDefinition {
                        name: schema.name,
                        description: schema.description,
                        parameters: schema.parameters,
                    },
                }
            })
            .collect()
    }

    /// Run a single turn: user message → LLM response → tool execution loop → final answer.
    ///
    /// Returns the final assistant response text.
    pub async fn run_turn(&mut self, user_message: String) -> Result<String, LoopError> {
        // ── Open turn ──────────────────────────────────────────────
        self.current_turn += 1;
        let turn = self.current_turn;
        self.state = TurnState::ClaimingInput;

        let seq = self.next_seq();
        self.session_log.push(SessionEvent::TurnStart {
            seq,
            time: now_ms(),
            turn,
        });

        let seq = self.next_seq();
        let time = now_ms();
        let content = user_message;
        self.session_log.push(SessionEvent::UserMessage {
            seq,
            time,
            content,
            turn: Some(turn),
            source: None,
        });

        let mut iteration = 0;
        let mut step = 0u64;

        loop {
            iteration += 1;
            if iteration > self.max_iterations {
                let reason = TurnEndReason::Error {
                    error: dsh_core_session::event::LlmFailure {
                        message: format!("max iterations ({}) exceeded", self.max_iterations),
                        code: "MAX_ITERATIONS".into(),
                    },
                };
                let seq = self.next_seq();
                let time = now_ms();
                let reason_c = reason.clone();
                self.session_log.push(SessionEvent::TurnEnd {
                    seq,
                    time,
                    turn,
                    reason: reason_c,
                });
                return Err(LoopError::Llm(dsh_llm_adapter::LlmError::Api(format!(
                    "max iterations ({}) exceeded",
                    self.max_iterations
                ))));
            }

            // ── Open step ──────────────────────────────────────────
            step += 1;
            let seq = self.next_seq();
            let time = now_ms();
            self.session_log.push(SessionEvent::StepStart {
                seq,
                time,
                turn,
                step,
            });

            // Assemble prompt
            self.state = TurnState::AssemblingPrompt;
            let mut messages = self.assemble_prompt();
            let mut tools = if self.tools.is_empty() {
                None
            } else {
                Some(self.build_tool_definitions())
            };

            // Run pre-step hooks
            self.hooks.run_pre_step(&mut PreStepCtx {
                turn,
                step,
                messages: &mut messages,
                tools: &mut tools,
                iteration,
            })?;

            // Build request
            let mut request = LlmRequest {
                model: self.model.clone(),
                messages,
                max_tokens: None,
                temperature: None,
                stream: true,
                tools,
            };

            // Run pre-request hooks
            self.hooks.run_pre_request(&mut PreRequestCtx {
                request: &mut request,
            })?;

            // Stream LLM response
            self.state = TurnState::Streaming;

            let mut stream = self.llm.stream(request).await?;
            let mut full_content = String::new();
            let mut tool_calls_by_index: HashMap<usize, (String, String, String)> = HashMap::new();

            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                full_content.push_str(&chunk.delta);
                if let Some(calls) = chunk.tool_calls {
                    for call in calls {
                        let index = call
                            .id
                            .strip_prefix("call_")
                            .and_then(|s| s.parse::<usize>().ok())
                            .unwrap_or(0);
                        let entry = tool_calls_by_index
                            .entry(index)
                            .or_insert_with(|| {
                                (call.id.clone(), call.function.name.clone(), String::new())
                            });
                        entry.2.push_str(&call.function.arguments);
                    }
                }
                let seq = self.next_seq();
                let time = now_ms();
                let delta = chunk.delta;
                self.session_log.push(SessionEvent::AssistantChunk {
                    seq,
                    time,
                    turn,
                    step,
                    content: delta,
                });
            }

            // Update token usage
            let completion_tokens = estimate_tokens(&full_content) as u64;
            self.token_usage.completion_tokens += completion_tokens;
            self.token_usage.total_tokens += completion_tokens;

            // Build final tool calls from accumulated data
            let tool_calls: Vec<LlmToolCall> = tool_calls_by_index
                .into_values()
                .map(|(id, name, args)| LlmToolCall {
                    id,
                    call_type: "function".into(),
                    function: FunctionCall { name, arguments: args },
                })
                .collect();

            // Check for structured tool calls
            if !tool_calls.is_empty() {
                // Collect chunk seqs before pushing
                let chunk_seqs: Vec<u64> = self
                    .session_log
                    .iter()
                    .rev()
                    .take_while(|e| matches!(e, SessionEvent::AssistantChunk { .. }))
                    .map(|e| e.seq())
                    .collect();

                let seq = self.next_seq();
                let time = now_ms();
                self.session_log.push(SessionEvent::AssistantMessage {
                    seq,
                    time,
                    turn,
                    step,
                    message: full_content,
                    usage: None,
                    source_event_seqs: Some(chunk_seqs),
                });

                // Execute each tool call
                for call in &tool_calls {
                    self.state = TurnState::ExecutingTools {
                        tool_name: call.function.name.clone(),
                    };

                    let seq = self.next_seq();
                    let time = now_ms();
                    let call_id = call.id.clone();
                    let name = call.function.name.clone();
                    let arguments = call.function.arguments.clone();
                    self.session_log.push(SessionEvent::ToolCall {
                        seq,
                        time,
                        turn,
                        step,
                        call_id,
                        name,
                        arguments,
                    });

                    let args_json: serde_json::Value =
                        serde_json::from_str(&call.function.arguments).unwrap_or_default();

                    // Run pre-execute hooks
                    let mut approved = true;
                    self.hooks.run_pre_execute(&mut PreExecuteCtx {
                        tool_name: &call.function.name,
                        arguments: &args_json,
                        approved: &mut approved,
                    })?;

                    let result = if !approved {
                        dsh_core_tools::ToolResult {
                            content: "Tool execution denied".into(),
                            is_error: true,
                        }
                    } else if let Some(tool) = self.tools.get(&call.function.name) {
                        tool.execute(args_json.clone()).await?
                    } else {
                        dsh_core_tools::ToolResult {
                            content: format!("tool not found: {}", call.function.name),
                            is_error: true,
                        }
                    };

                    // Run post-execute hooks
                    self.hooks.run_post_execute(&PostExecuteCtx {
                        tool_name: &call.function.name,
                        arguments: &args_json,
                        result: &result,
                    });

                    let seq = self.next_seq();
                    let time = now_ms();
                    let call_id = call.id.clone();
                    let content = result.content;
                    let is_error = result.is_error;
                    self.session_log.push(SessionEvent::ToolResult {
                        seq,
                        time,
                        turn,
                        step,
                        call_id,
                        content,
                        is_error,
                        meta: None,
                        source_event_seqs: None,
                    });
                }

                // ── Close step (tool call step) ────────────────────
                let seq = self.next_seq();
                let time = now_ms();
                self.session_log.push(SessionEvent::StepEnd {
                    seq,
                    time,
                    turn,
                    step,
                });

                // Loop back to assemble prompt with the new tool results
                continue;
            }

            // ── No tool call — final answer ────────────────────────
            let chunk_seqs: Vec<u64> = self
                .session_log
                .iter()
                .rev()
                .take_while(|e| matches!(e, SessionEvent::AssistantChunk { .. }))
                .map(|e| e.seq())
                .collect();

            let seq = self.next_seq();
            let time = now_ms();
            self.session_log.push(SessionEvent::AssistantMessage {
                seq,
                time,
                turn,
                step,
                message: full_content.clone(),
                usage: None,
                source_event_seqs: Some(chunk_seqs),
            });

            // ── Close step ─────────────────────────────────────────
            let seq = self.next_seq();
            let time = now_ms();
            self.session_log.push(SessionEvent::StepEnd {
                seq,
                time,
                turn,
                step,
            });

            // ── Close turn ─────────────────────────────────────────
            let seq = self.next_seq();
            let time = now_ms();
            self.session_log.push(SessionEvent::TurnEnd {
                seq,
                time,
                turn,
                reason: TurnEndReason::Completed,
            });

            self.state = TurnState::Closed;
            return Ok(full_content);
        }
    }

    /// Assemble the prompt from the session log.
    fn assemble_prompt(&self) -> Vec<Message> {
        let mut messages = Vec::new();

        // Inject system prompt if set
        let assembled = self.system_prompt.assemble();
        if !assembled.is_empty() {
            messages.push(Message {
                role: dsh_llm_adapter::Role::System,
                content: assembled,
                tool_calls: None,
                tool_call_id: None,
            });
        }

        // Process session events to build proper messages with tool calls
        let mut current_tool_calls: Vec<dsh_llm_adapter::ToolCall> = Vec::new();
        let mut current_content = String::new();
        let mut in_tool_call = false;

        for event in &self.session_log {
            match event {
                SessionEvent::UserMessage { content, .. } => {
                    messages.push(Message {
                        role: dsh_llm_adapter::Role::User,
                        content: content.clone(),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
                SessionEvent::AssistantMessage { message, .. } => {
                    if !current_tool_calls.is_empty() {
                        messages.push(Message {
                            role: dsh_llm_adapter::Role::Assistant,
                            content: current_content.clone(),
                            tool_calls: Some(current_tool_calls.clone()),
                            tool_call_id: None,
                        });
                        current_tool_calls.clear();
                        current_content.clear();
                        in_tool_call = false;
                    } else {
                        messages.push(Message {
                            role: dsh_llm_adapter::Role::Assistant,
                            content: message.clone(),
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    }
                }
                SessionEvent::ToolCall {
                    call_id,
                    name,
                    arguments,
                    ..
                } => {
                    in_tool_call = true;
                    current_tool_calls.push(dsh_llm_adapter::ToolCall {
                        id: call_id.clone(),
                        call_type: "function".into(),
                        function: dsh_llm_adapter::FunctionCall {
                            name: name.clone(),
                            arguments: arguments.clone(),
                        },
                    });
                }
                SessionEvent::ToolResult {
                    call_id, content, ..
                } => {
                    if !current_tool_calls.is_empty() {
                        messages.push(Message {
                            role: dsh_llm_adapter::Role::Assistant,
                            content: current_content.clone(),
                            tool_calls: Some(current_tool_calls.clone()),
                            tool_call_id: None,
                        });
                        current_tool_calls.clear();
                        current_content.clear();
                        in_tool_call = false;
                    }
                    messages.push(Message {
                        role: dsh_llm_adapter::Role::Tool,
                        content: content.clone(),
                        tool_calls: None,
                        tool_call_id: Some(call_id.clone()),
                    });
                }
                SessionEvent::AssistantChunk { content, .. } => {
                    if !in_tool_call {
                        current_content.push_str(content);
                    }
                }
                // Lifecycle events are not model-visible.
                SessionEvent::TurnStart { .. }
                | SessionEvent::TurnEnd { .. }
                | SessionEvent::StepStart { .. }
                | SessionEvent::StepEnd { .. }
                | SessionEvent::RequestHeader { .. }
                | SessionEvent::EndSeed { .. } => {}
            }
        }

        // Flush any remaining tool calls
        if !current_tool_calls.is_empty() {
            messages.push(Message {
                role: dsh_llm_adapter::Role::Assistant,
                content: current_content,
                tool_calls: Some(current_tool_calls),
                tool_call_id: None,
            });
        }

        messages
    }

    /// Estimate token count for the current session.
    pub fn estimate_tokens(&self) -> usize {
        let mut total = 0;
        let assembled = self.system_prompt.assemble();
        if !assembled.is_empty() {
            total += estimate_tokens(&assembled);
        }
        for event in &self.session_log {
            match event {
                SessionEvent::UserMessage { content, .. }
                | SessionEvent::AssistantChunk { content, .. } => {
                    total += estimate_tokens(content);
                }
                SessionEvent::AssistantMessage { message, .. } => {
                    total += estimate_tokens(message);
                }
                SessionEvent::ToolResult { content, .. } => {
                    total += estimate_tokens(content);
                }
                _ => {}
            }
        }
        total
    }

    /// Check if context window is exceeded.
    pub fn is_context_exceeded(&self) -> bool {
        self.estimate_tokens() > self.max_context_tokens
    }

    /// Run multiple turns of conversation.
    pub async fn run_conversation(
        &mut self,
        messages: Vec<String>,
    ) -> Result<Vec<String>, LoopError> {
        let mut responses = Vec::new();
        for (i, msg) in messages.into_iter().enumerate() {
            if i >= self.max_turns {
                break;
            }
            let response = self.run_turn(msg).await?;
            responses.push(response);
        }
        Ok(responses)
    }

    /// Export session log as JSON.
    pub fn export_session(&self) -> Result<String, LoopError> {
        serde_json::to_string_pretty(&self.session_log).map_err(|e| {
            LoopError::Llm(dsh_llm_adapter::LlmError::Api(format!(
                "serialize error: {e}"
            )))
        })
    }

    /// Import session log from JSON.
    pub fn import_session(&mut self, json: &str) -> Result<(), LoopError> {
        let log: Vec<SessionEvent> = serde_json::from_str(json).map_err(|e| {
            LoopError::Llm(dsh_llm_adapter::LlmError::Api(format!(
                "deserialize error: {e}"
            )))
        })?;
        // Re-derive seq from the last event.
        if let Some(last) = log.last() {
            self.seq = last.seq();
        }
        // Re-derive current_turn from TurnStart events.
        self.current_turn = log
            .iter()
            .filter_map(|e| match e {
                SessionEvent::TurnStart { turn, .. } => Some(*turn),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        self.session_log = log;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_tool_definitions_empty() {
        struct MockLlm;
        #[async_trait::async_trait]
        impl LlmAdapter for MockLlm {
            async fn stream(
                &self,
                _request: LlmRequest,
            ) -> Result<
                std::pin::Pin<
                    Box<
                        dyn futures::Stream<
                                Item = Result<dsh_llm_adapter::LlmChunk, dsh_llm_adapter::LlmError>,
                            > + Send,
                    >,
                >,
                dsh_llm_adapter::LlmError,
            > {
                unimplemented!()
            }
            async fn complete(
                &self,
                _request: LlmRequest,
            ) -> Result<dsh_llm_adapter::LlmResponse, dsh_llm_adapter::LlmError> {
                unimplemented!()
            }
        }

        let runner = AgentRunner::new(Arc::new(MockLlm), "test-model".into());
        assert!(runner.build_tool_definitions().is_empty());
    }
}