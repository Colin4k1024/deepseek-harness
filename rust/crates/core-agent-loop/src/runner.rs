use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;

use dsh_core_session::event::SessionEvent;
use dsh_core_tools::Tool;
use dsh_llm_adapter::{
    FunctionCall, FunctionDefinition, LlmAdapter, LlmRequest, Message, ToolCall as LlmToolCall,
    ToolDefinition,
};

use crate::state::{LoopError, TurnState};

/// Token counting estimate (4 chars ≈ 1 token).
fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
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
/// Streaming → ExecutingTools → Closed.
pub struct AgentRunner {
    llm: Arc<dyn LlmAdapter>,
    tools: HashMap<String, Arc<dyn Tool>>,
    session_log: Vec<SessionEvent>,
    state: TurnState,
    model: String,
    system_prompt: Option<String>,
    max_context_tokens: usize,
    max_turns: usize,
    max_iterations: usize,
    token_usage: TokenUsage,
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
            system_prompt: None,
            max_context_tokens: 128000,
            max_turns: 10,
            max_iterations: 20,
            token_usage: TokenUsage::default(),
        }
    }

    /// Set the system prompt.
    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        self.system_prompt = Some(prompt);
        self
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
        // Claim input
        self.state = TurnState::ClaimingInput;
        self.session_log.push(SessionEvent::UserMessage {
            content: user_message,
        });

        let mut iteration = 0;
        loop {
            iteration += 1;
            if iteration > self.max_iterations {
                return Err(LoopError::Llm(dsh_llm_adapter::LlmError::Api(format!(
                    "max iterations ({}) exceeded",
                    self.max_iterations
                ))));
            }

            // Assemble prompt
            self.state = TurnState::AssemblingPrompt;
            let messages = self.assemble_prompt();

            // Build tools parameter
            let tools = if self.tools.is_empty() {
                None
            } else {
                Some(self.build_tool_definitions())
            };

            // Estimate prompt tokens before moving messages
            let _prompt_tokens: u64 = messages
                .iter()
                .map(|m| estimate_tokens(&m.content) as u64)
                .sum();

            // Stream LLM response
            self.state = TurnState::Streaming;
            let request = LlmRequest {
                model: self.model.clone(),
                messages,
                max_tokens: None,
                temperature: None,
                stream: true,
                tools,
            };

            let mut stream = self.llm.stream(request).await?;
            let mut full_content = String::new();
            let mut tool_calls_by_index: std::collections::HashMap<
                usize,
                (String, String, String),
            > = std::collections::HashMap::new();

            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                full_content.push_str(&chunk.delta);
                if let Some(calls) = chunk.tool_calls {
                    for call in calls {
                        // Extract index from id (format: "call_N" or just use 0)
                        let index = call
                            .id
                            .strip_prefix("call_")
                            .and_then(|s| s.parse::<usize>().ok())
                            .unwrap_or(0);
                        let entry = tool_calls_by_index.entry(index).or_insert_with(|| {
                            (call.id.clone(), call.function.name.clone(), String::new())
                        });
                        // Accumulate arguments
                        entry.2.push_str(&call.function.arguments);
                    }
                }
                self.session_log.push(SessionEvent::AssistantChunk {
                    content: chunk.delta,
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
                    function: FunctionCall {
                        name,
                        arguments: args,
                    },
                })
                .collect();

            // Check for structured tool calls
            if !tool_calls.is_empty() {
                // Record the assistant's tool-call message
                self.session_log.push(SessionEvent::AssistantMessage {
                    content: full_content,
                });

                // Execute each tool call
                for call in &tool_calls {
                    self.state = TurnState::ExecutingTools {
                        tool_name: call.function.name.clone(),
                    };

                    let arguments: serde_json::Value =
                        serde_json::from_str(&call.function.arguments).unwrap_or_default();

                    self.session_log.push(SessionEvent::ToolCall {
                        id: call.id.clone(),
                        name: call.function.name.clone(),
                        arguments: arguments.clone(),
                    });

                    let result = if let Some(tool) = self.tools.get(&call.function.name) {
                        tool.execute(arguments).await?
                    } else {
                        dsh_core_tools::ToolResult {
                            content: format!("tool not found: {}", call.function.name),
                            is_error: true,
                        }
                    };

                    self.session_log.push(SessionEvent::ToolResult {
                        call_id: call.id.clone(),
                        content: result.content,
                    });
                }

                // Loop back to assemble prompt with the new tool results
                continue;
            }

            // No tool call — this is the final answer
            self.session_log.push(SessionEvent::AssistantMessage {
                content: full_content.clone(),
            });
            self.state = TurnState::Closed;
            return Ok(full_content);
        }
    }

    /// Assemble the prompt from the session log.
    fn assemble_prompt(&self) -> Vec<Message> {
        let mut messages = Vec::new();

        // Inject system prompt if set
        if let Some(ref system_prompt) = self.system_prompt {
            messages.push(Message {
                role: dsh_llm_adapter::Role::System,
                content: system_prompt.clone(),
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
                SessionEvent::UserMessage { content } => {
                    messages.push(Message {
                        role: dsh_llm_adapter::Role::User,
                        content: content.clone(),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
                SessionEvent::AssistantMessage { content } => {
                    // Flush any pending tool calls
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
                            content: content.clone(),
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    }
                }
                SessionEvent::ToolCall {
                    id,
                    name,
                    arguments,
                } => {
                    in_tool_call = true;
                    current_tool_calls.push(dsh_llm_adapter::ToolCall {
                        id: id.clone(),
                        call_type: "function".into(),
                        function: dsh_llm_adapter::FunctionCall {
                            name: name.clone(),
                            arguments: serde_json::to_string(arguments).unwrap_or_default(),
                        },
                    });
                }
                SessionEvent::ToolResult { call_id, content } => {
                    // First flush the assistant message with tool calls
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
                    // Then add the tool result
                    messages.push(Message {
                        role: dsh_llm_adapter::Role::Tool,
                        content: content.clone(),
                        tool_calls: None,
                        tool_call_id: Some(call_id.clone()),
                    });
                }
                SessionEvent::AssistantChunk { content } => {
                    if in_tool_call {
                        // Don't accumulate chunks during tool calls
                    } else {
                        current_content.push_str(content);
                    }
                }
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
        if let Some(ref sp) = self.system_prompt {
            total += estimate_tokens(sp);
        }
        for event in &self.session_log {
            match event {
                SessionEvent::UserMessage { content }
                | SessionEvent::AssistantMessage { content }
                | SessionEvent::AssistantChunk { content } => {
                    total += estimate_tokens(content);
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
        self.session_log = log;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_tool_definitions_empty() {
        // Create a mock LLM adapter
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
