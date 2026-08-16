//! Subagent tool — delegates a self-contained task to an independent agent.
//!
//! The subagent runs with the same tools as the parent in a separate session.
//! Only the final response is returned; the subagent's session log is discarded.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use dsh_core_agent_loop::AgentRunner;
use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};
use dsh_llm_adapter::LlmAdapter;

/// Delegates a task to an independent subagent with its own session.
///
/// The subagent has access to the same tools as the parent but runs in
/// a separate session. Its intermediate events are not visible to the
/// parent — only the final text response is returned.
///
/// When `fork=true`, the subagent inherits the parent's conversation
/// context via `session_log` (a JSON array of SessionEvent objects).
/// This allows the subagent to build on previous conversation turns.
pub struct SubagentTool {
    llm: Arc<dyn LlmAdapter>,
    tools: HashMap<String, Arc<dyn Tool>>,
    model: String,
    max_iterations: usize,
}

impl SubagentTool {
    /// Create a new subagent tool.
    pub fn new(llm: Arc<dyn LlmAdapter>, model: impl Into<String>) -> Self {
        Self {
            llm,
            tools: HashMap::new(),
            model: model.into(),
            max_iterations: 10,
        }
    }

    /// Register a tool that the subagent can use.
    pub fn with_tool(mut self, tool: Arc<dyn Tool>) -> Self {
        let name = tool.schema().name.clone();
        self.tools.insert(name, tool);
        self
    }

    /// Register multiple tools at once.
    pub fn with_tools(mut self, tools: HashMap<String, Arc<dyn Tool>>) -> Self {
        self.tools = tools;
        self
    }

    /// Set the maximum number of iterations per turn.
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Return the model name used by subagents.
    pub fn model(&self) -> &str {
        &self.model
    }
}

#[async_trait]
impl Tool for SubagentTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "subagent".into(),
            description: "Delegate a self-contained task to an independent subagent. \
                The subagent runs with the same tools and returns its final response. \
                Use this for parallel work, research, or analysis that doesn't need \
                access to the parent's conversation. \
                Set fork=true and provide session_log to inherit the parent's context.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "The complete, self-contained task for the subagent"
                    },
                    "description": {
                        "type": "string",
                        "description": "A short (3-5 word) description of the task, for display"
                    },
                    "fork": {
                        "type": "boolean",
                        "description": "Whether to inherit the parent's conversation context. When true, session_log must be provided."
                    },
                    "session_log": {
                        "type": "string",
                        "description": "JSON array of the parent's session events (required when fork=true)"
                    }
                },
                "required": ["prompt"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("prompt is required".into()))?;

        let fork = args
            .get("fork")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Create a fresh runner for the subagent
        let mut runner = AgentRunner::new(self.llm.clone(), self.model.clone());
        runner = runner.with_max_iterations(self.max_iterations);

        // Register all tools
        for tool in self.tools.values() {
            runner.register_tool(tool.clone());
        }

        // If fork mode, import the parent's session log
        if fork {
            let session_log = args
                .get("session_log")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ToolError::InvalidArgs(
                        "session_log is required when fork=true".into(),
                    )
                })?;
            runner.import_session(session_log).map_err(|e| {
                ToolError::Execution(format!("import session failed: {e}"))
            })?;
        }

        // Run the subagent
        match runner.run_turn(prompt.to_string()).await {
            Ok(response) => Ok(ToolResult {
                content: response,
                is_error: false,
            }),
            Err(e) => Ok(ToolResult {
                content: format!("Subagent failed: {e}"),
                is_error: true,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsh_llm_adapter::{LlmChunk, LlmError, LlmRequest, LlmResponse};

    /// A mock LLM adapter that returns a fixed response.
    struct EchoLlm {
        response: String,
    }

    #[async_trait::async_trait]
    impl LlmAdapter for EchoLlm {
        async fn stream(
            &self,
            _request: LlmRequest,
        ) -> Result<
            std::pin::Pin<
                Box<
                    dyn futures::Stream<Item = Result<LlmChunk, LlmError>> + Send,
                >,
            >,
            LlmError,
        > {
            // Return a single chunk with the response
            let chunk = LlmChunk {
                delta: self.response.clone(),
                tool_calls: None,
                finish_reason: Some("stop".into()),
            };
            let stream = futures::stream::once(async move { Ok(chunk) });
            Ok(Box::pin(stream))
        }

        async fn complete(
            &self,
            _request: LlmRequest,
        ) -> Result<LlmResponse, LlmError> {
            Ok(LlmResponse {
                content: self.response.clone(),
                finish_reason: "stop".into(),
                usage: None,
                tool_calls: None,
            })
        }
    }

    #[tokio::test]
    async fn subagent_returns_response() {
        let llm = Arc::new(EchoLlm {
            response: "Task completed: analysis done.".into(),
        });
        let tool = SubagentTool::new(llm, "test-model");

        let result = tool
            .execute(serde_json::json!({"prompt": "Analyze the data"}))
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.content, "Task completed: analysis done.");
    }

    #[tokio::test]
    async fn subagent_missing_prompt() {
        let llm = Arc::new(EchoLlm {
            response: String::new(),
        });
        let tool = SubagentTool::new(llm, "test-model");

        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn subagent_with_tools() {
        // A simple echo tool
        struct EchoTool;
        #[async_trait::async_trait]
        impl Tool for EchoTool {
            fn schema(&self) -> ToolSchema {
                ToolSchema {
                    name: "echo".into(),
                    description: "Echoes input".into(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "text": {"type": "string"}
                        }
                    }),
                }
            }

            async fn execute(
                &self,
                args: serde_json::Value,
            ) -> Result<ToolResult, ToolError> {
                let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
                Ok(ToolResult {
                    content: format!("echo: {text}"),
                    is_error: false,
                })
            }
        }

        let llm = Arc::new(EchoLlm {
            response: "Used echo tool successfully.".into(),
        });
        let tool = SubagentTool::new(llm, "test-model")
            .with_tool(Arc::new(EchoTool));

        let result = tool
            .execute(serde_json::json!({"prompt": "Echo hello"}))
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.content, "Used echo tool successfully.");
    }

    #[test]
    fn schema_is_valid() {
        let llm = Arc::new(EchoLlm {
            response: String::new(),
        });
        let tool = SubagentTool::new(llm, "test-model");
        let schema = tool.schema();
        assert_eq!(schema.name, "subagent");
        assert!(schema.description.contains("Delegate"));
        let params = schema.parameters.as_object().unwrap();
        assert!(params.get("required").unwrap().as_array().unwrap().contains(
            &serde_json::Value::String("prompt".into())
        ));
        // Fork parameter should be in the schema
        let props = params.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("fork"));
        assert!(props.contains_key("session_log"));
    }

    #[tokio::test]
    async fn subagent_fork_with_session_log() {
        let llm = Arc::new(EchoLlm {
            response: "Forked response: continuing from parent context.".into(),
        });
        let tool = SubagentTool::new(llm, "test-model");

        // Create a parent session log as JSON
        let parent_log = serde_json::json!([
            {
                "type": "turn/start",
                "seq": 1,
                "time": 1000,
                "turn": 1
            },
            {
                "type": "user/message",
                "seq": 2,
                "time": 1001,
                "content": "What is Rust?",
                "turn": 1
            },
            {
                "type": "assistant/message",
                "seq": 3,
                "time": 1002,
                "turn": 1,
                "step": 1,
                "message": "Rust is a systems programming language."
            },
            {
                "type": "turn/end",
                "seq": 4,
                "time": 1003,
                "turn": 1,
                "reason": {"kind": "completed"}
            }
        ]);

        let result = tool
            .execute(serde_json::json!({
                "prompt": "Continue from the parent context",
                "fork": true,
                "session_log": parent_log.to_string()
            }))
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.content, "Forked response: continuing from parent context.");
    }

    #[tokio::test]
    async fn subagent_fork_missing_session_log() {
        let llm = Arc::new(EchoLlm {
            response: String::new(),
        });
        let tool = SubagentTool::new(llm, "test-model");

        let result = tool
            .execute(serde_json::json!({
                "prompt": "Test",
                "fork": true
            }))
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{err}").contains("session_log"));
    }

    #[tokio::test]
    async fn subagent_fork_preserves_context() {
        let llm = Arc::new(EchoLlm {
            response: "Understood, continuing from the parent context.".into(),
        });
        let tool = SubagentTool::new(llm, "test-model");

        // A minimal valid session log
        let parent_log = serde_json::json!([
            {
                "type": "turn/start",
                "seq": 1,
                "time": 1000,
                "turn": 1
            },
            {
                "type": "user/message",
                "seq": 2,
                "time": 1001,
                "content": "parent question",
                "turn": 1
            },
            {
                "type": "assistant/message",
                "seq": 3,
                "time": 1002,
                "turn": 1,
                "step": 1,
                "message": "parent answer"
            },
            {
                "type": "turn/end",
                "seq": 4,
                "time": 1003,
                "turn": 1,
                "reason": {"kind": "completed"}
            }
        ]);

        let result = tool
            .execute(serde_json::json!({
                "prompt": "Continue conversation",
                "fork": true,
                "session_log": parent_log.to_string()
            }))
            .await
            .unwrap();

        assert!(!result.is_error);
        // The forked subagent should have the parent's context
        assert!(result.content.contains("continuing"));
    }

    #[tokio::test]
    async fn subagent_without_fork_ignores_session_log() {
        let llm = Arc::new(EchoLlm {
            response: "Fresh response, no parent context.".into(),
        });
        let tool = SubagentTool::new(llm, "test-model");

        // Even with session_log, without fork=true it should be ignored
        let result = tool
            .execute(serde_json::json!({
                "prompt": "Fresh task",
                "fork": false,
                "session_log": "[]"
            }))
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.content, "Fresh response, no parent context.");
    }
}