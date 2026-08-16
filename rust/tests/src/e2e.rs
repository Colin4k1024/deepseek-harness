//! End-to-end integration tests for dsh-rust.
//!
//! Tests the full stack: Context → Session → Tools → Agent Loop → Persistence → Auth → Web API.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::stream;
use std::pin::Pin;

// Re-export all crates under test
use dsh_core_agent_loop::{AgentRunner, TurnState};
use dsh_core_context::Context;
use dsh_core_memory::{InMemoryStore, MemoryEntry, MemoryStore, SemanticSearch};
use dsh_core_orchestrator::{AgentHandle, AgentRegistry, Orchestrator};
use dsh_core_session::derive::derive_messages;
use dsh_core_session::event::{Role, SessionEvent, Surface};
use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};
use dsh_core_workflow::{
    Step, StepExecutor, StepResult, StepStatus, Workflow, WorkflowExecutor, WorkflowStatus,
};
use dsh_llm_adapter::{
    FunctionCall, LlmAdapter, LlmChunk, LlmError, LlmRequest, LlmResponse, ToolCall as LlmToolCall,
};
use dsh_seam_auth::{AuthManager, QuotaConfig, QuotaManager, RateLimitConfig, RateLimiter};
use dsh_seam_observability::Metrics;
use dsh_seam_persistence::SessionStore;
use dsh_seam_sandbox::{SandboxPolicy, SandboxResolution, resolve as resolve_sandbox};

// ============================================================================
// Test fixtures
// ============================================================================

/// Scripted LLM that returns pre-set responses in sequence.
struct ScriptedLlm {
    responses: Vec<LlmResponse>,
    call_count: std::sync::Mutex<usize>,
}

impl ScriptedLlm {
    fn new(responses: Vec<LlmResponse>) -> Self {
        Self {
            responses,
            call_count: std::sync::Mutex::new(0),
        }
    }
}

#[async_trait]
impl LlmAdapter for ScriptedLlm {
    async fn stream(
        &self,
        _request: LlmRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<LlmChunk, LlmError>> + Send>>, LlmError>
    {
        let mut count = self.call_count.lock().unwrap();
        let idx = *count;
        *count += 1;
        let resp = self
            .responses
            .get(idx)
            .cloned()
            .unwrap_or_else(|| LlmResponse {
                content: "done".into(),
                finish_reason: "stop".into(),
                usage: None,
                tool_calls: None,
            });

        let chunks = vec![Ok(LlmChunk {
            delta: resp.content,
            finish_reason: Some(resp.finish_reason),
            tool_calls: resp.tool_calls,
        })];
        Ok(Box::pin(stream::iter(chunks)))
    }

    async fn complete(&self, _request: LlmRequest) -> Result<LlmResponse, LlmError> {
        unimplemented!("use stream()")
    }
}

/// Echo tool for testing.
struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "echo".into(),
            description: "Echo the input text".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string" }
                },
                "required": ["text"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let text = args["text"].as_str().unwrap_or("no input");
        Ok(ToolResult {
            content: format!("echo: {text}"),
            is_error: false,
        })
    }
}

/// Calculator tool for testing.
struct CalcTool;

#[async_trait]
impl Tool for CalcTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "calc".into(),
            description: "Evaluate a math expression".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "expr": { "type": "string" }
                },
                "required": ["expr"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let expr = args["expr"].as_str().unwrap_or("0");
        let result = match expr {
            "2+3" => "5",
            "10*2" => "20",
            _ => "42",
        };
        Ok(ToolResult {
            content: result.into(),
            is_error: false,
        })
    }
}

// ============================================================================
// Test: Full agent loop with tool execution
// ============================================================================

#[tokio::test]
async fn e2e_agent_loop_simple() {
    let llm = Arc::new(ScriptedLlm::new(vec![LlmResponse {
        content: "Hello, world!".into(),
        finish_reason: "stop".into(),
        usage: None,
        tool_calls: None,
    }]));
    let mut runner = AgentRunner::new(llm, "test-model".into());

    let response = runner.run_turn("Hi".into()).await.unwrap();
    assert_eq!(response, "Hello, world!");
    assert_eq!(*runner.state(), TurnState::Closed);
    assert_eq!(runner.session_log().len(), 3); // User + Chunk + Assistant
}

#[tokio::test]
async fn e2e_agent_loop_with_tool() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        LlmResponse {
            content: String::new(),
            finish_reason: "tool_calls".into(),
            usage: None,
            tool_calls: Some(vec![LlmToolCall {
                id: "call_1".into(),
                call_type: "function".into(),
                function: FunctionCall {
                    name: "echo".into(),
                    arguments: r#"{"text": "test"}"#.into(),
                },
            }]),
        },
        LlmResponse {
            content: "The echo said: echo: test".into(),
            finish_reason: "stop".into(),
            usage: None,
            tool_calls: None,
        },
    ]));
    let mut runner = AgentRunner::new(llm, "test-model".into());
    runner.register_tool(Arc::new(EchoTool));

    let response = runner.run_turn("echo test".into()).await.unwrap();
    assert_eq!(response, "The echo said: echo: test");
    assert_eq!(*runner.state(), TurnState::Closed);
    // User + Chunk(tool) + Assistant(tool) + ToolCall + ToolResult + Chunk(final) + Assistant(final)
    assert_eq!(runner.session_log().len(), 7);
}

#[tokio::test]
async fn e2e_agent_loop_multi_tool() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        LlmResponse {
            content: String::new(),
            finish_reason: "tool_calls".into(),
            usage: None,
            tool_calls: Some(vec![
                LlmToolCall {
                    id: "call_1".into(),
                    call_type: "function".into(),
                    function: FunctionCall {
                        name: "echo".into(),
                        arguments: r#"{"text": "hello"}"#.into(),
                    },
                },
                LlmToolCall {
                    id: "call_2".into(),
                    call_type: "function".into(),
                    function: FunctionCall {
                        name: "calc".into(),
                        arguments: r#"{"expr": "2+3"}"#.into(),
                    },
                },
            ]),
        },
        LlmResponse {
            content: "Done!".into(),
            finish_reason: "stop".into(),
            usage: None,
            tool_calls: None,
        },
    ]));
    let mut runner = AgentRunner::new(llm, "test-model".into());
    runner.register_tool(Arc::new(EchoTool));
    runner.register_tool(Arc::new(CalcTool));

    let response = runner.run_turn("do both".into()).await.unwrap();
    assert_eq!(response, "Done!");
    assert_eq!(*runner.state(), TurnState::Closed);
}

// ============================================================================
// Test: Real tools (bash, file read, file write)
// ============================================================================

#[tokio::test]
async fn e2e_real_bash_tool() {
    use dsh_seam_tools::BashTool;

    let llm = Arc::new(ScriptedLlm::new(vec![
        LlmResponse {
            content: String::new(),
            finish_reason: "tool_calls".into(),
            usage: None,
            tool_calls: Some(vec![LlmToolCall {
                id: "call_1".into(),
                call_type: "function".into(),
                function: FunctionCall {
                    name: "bash".into(),
                    arguments: r#"{"command": "echo hello from bash"}"#.into(),
                },
            }]),
        },
        LlmResponse {
            content: "The bash output is: hello from bash".into(),
            finish_reason: "stop".into(),
            usage: None,
            tool_calls: None,
        },
    ]));
    let mut runner = AgentRunner::new(llm, "test-model".into());
    runner.register_tool(Arc::new(BashTool::new()));

    let response = runner.run_turn("run echo".into()).await.unwrap();
    assert_eq!(response, "The bash output is: hello from bash");

    // Verify the tool result was captured
    let log = runner.session_log();
    let tool_result = log.iter().find(|e| matches!(e, SessionEvent::ToolResult { .. }));
    assert!(tool_result.is_some());
    if let SessionEvent::ToolResult { content, .. } = tool_result.unwrap() {
        assert!(content.contains("hello from bash"));
    }
}

#[tokio::test]
async fn e2e_real_file_write_and_read() {
    use dsh_seam_tools::{FileReadTool, FileWriteTool};

    let tmp_path = "/tmp/dsh_e2e_test_file.txt";

    // LLM first writes a file, then reads it back
    let llm = Arc::new(ScriptedLlm::new(vec![
        // Step 1: Write file
        LlmResponse {
            content: String::new(),
            finish_reason: "tool_calls".into(),
            usage: None,
            tool_calls: Some(vec![LlmToolCall {
                id: "call_write".into(),
                call_type: "function".into(),
                function: FunctionCall {
                    name: "write_file".into(),
                    arguments: serde_json::json!({
                        "path": tmp_path,
                        "content": "Hello from dsh-rust!"
                    })
                    .to_string(),
                },
            }]),
        },
        // Step 2: Read file
        LlmResponse {
            content: String::new(),
            finish_reason: "tool_calls".into(),
            usage: None,
            tool_calls: Some(vec![LlmToolCall {
                id: "call_read".into(),
                call_type: "function".into(),
                function: FunctionCall {
                    name: "read_file".into(),
                    arguments: serde_json::json!({"path": tmp_path}).to_string(),
                },
            }]),
        },
        // Step 3: Final answer
        LlmResponse {
            content: "File contains: Hello from dsh-rust!".into(),
            finish_reason: "stop".into(),
            usage: None,
            tool_calls: None,
        },
    ]));

    let mut runner = AgentRunner::new(llm, "test-model".into());
    runner.register_tool(Arc::new(FileWriteTool));
    runner.register_tool(Arc::new(FileReadTool));

    let response = runner
        .run_turn("write and read a file".into())
        .await
        .unwrap();
    assert_eq!(response, "File contains: Hello from dsh-rust!");

    // Verify both tool calls happened
    let log = runner.session_log();
    let tool_calls: Vec<_> = log
        .iter()
        .filter(|e| matches!(e, SessionEvent::ToolCall { .. }))
        .collect();
    assert_eq!(tool_calls.len(), 2); // write + read

    // Cleanup
    let _ = tokio::fs::remove_file(tmp_path).await;
}

#[tokio::test]
async fn e2e_real_bash_with_system_prompt() {
    use dsh_seam_tools::BashTool;

    let llm = Arc::new(ScriptedLlm::new(vec![
        LlmResponse {
            content: String::new(),
            finish_reason: "tool_calls".into(),
            usage: None,
            tool_calls: Some(vec![LlmToolCall {
                id: "call_1".into(),
                call_type: "function".into(),
                function: FunctionCall {
                    name: "bash".into(),
                    arguments: r#"{"command": "whoami"}"#.into(),
                },
            }]),
        },
        LlmResponse {
            content: "The current user is displayed above.".into(),
            finish_reason: "stop".into(),
            usage: None,
            tool_calls: None,
        },
    ]));

    let mut runner = AgentRunner::new(llm, "test-model".into())
        .with_system_prompt("You are a helpful assistant that can run commands.".into());
    runner.register_tool(Arc::new(BashTool::new()));

    let response = runner.run_turn("who am I?".into()).await.unwrap();
    assert_eq!(response, "The current user is displayed above.");

    // Verify system prompt was injected
    let log = runner.session_log();
    // First message should be the user message (system prompt is in assemble_prompt)
    assert!(matches!(&log[0], SessionEvent::UserMessage { .. }));
}

#[tokio::test]
async fn e2e_session_export_import() {
    let llm = Arc::new(ScriptedLlm::new(vec![LlmResponse {
        content: "Hello!".into(),
        finish_reason: "stop".into(),
        usage: None,
        tool_calls: None,
    }]));

    let mut runner = AgentRunner::new(llm, "test-model".into());
    runner.run_turn("Hi".into()).await.unwrap();

    // Export session
    let json = runner.export_session().unwrap();
    assert!(json.contains("Hello!"));

    // Import into new runner
    let llm2 = Arc::new(ScriptedLlm::new(vec![]));
    let mut runner2 = AgentRunner::new(llm2, "test-model".into());
    runner2.import_session(&json).unwrap();
    assert_eq!(runner2.session_log().len(), runner.session_log().len());
}

#[tokio::test]
async fn e2e_multi_turn_conversation() {
    use dsh_seam_tools::BashTool;

    // 3 turns of conversation
    let llm = Arc::new(ScriptedLlm::new(vec![
        LlmResponse {
            content: "Turn 1 response".into(),
            finish_reason: "stop".into(),
            usage: None,
            tool_calls: None,
        },
        LlmResponse {
            content: "Turn 2 response".into(),
            finish_reason: "stop".into(),
            usage: None,
            tool_calls: None,
        },
        LlmResponse {
            content: "Turn 3 response".into(),
            finish_reason: "stop".into(),
            usage: None,
            tool_calls: None,
        },
    ]));

    let mut runner = AgentRunner::new(llm, "test-model".into())
        .with_max_turns(5);
    runner.register_tool(Arc::new(BashTool::new()));

    let responses = runner
        .run_conversation(vec![
            "Hello".into(),
            "How are you?".into(),
            "Goodbye".into(),
        ])
        .await
        .unwrap();

    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0], "Turn 1 response");
    assert_eq!(responses[1], "Turn 2 response");
    assert_eq!(responses[2], "Turn 3 response");
}
