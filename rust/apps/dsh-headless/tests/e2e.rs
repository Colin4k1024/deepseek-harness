//! Integration tests for the headless runner end-to-end flow.

use std::sync::Arc;

use async_trait::async_trait;
use futures::stream;
use std::pin::Pin;

use dsh_core_agent_loop::AgentRunner;
use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};
use dsh_llm_adapter::{
    FunctionCall, LlmAdapter, LlmChunk, LlmError, LlmRequest, LlmResponse, ToolCall as LlmToolCall,
};
use dsh_seam_sandbox::{SandboxPolicy, SandboxResolution, resolve as resolve_sandbox};

/// Scripted LLM that returns pre-set responses.
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
        unimplemented!()
    }
}

/// A calculator tool for testing multi-step tool calls.
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
        // Simple eval for testing: just handle "2+3" style
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

#[tokio::test]
async fn sandbox_resolve_works() {
    let resolution = resolve_sandbox(SandboxPolicy::Minimal);
    // On macOS, should be Confined with seatbelt
    match resolution {
        SandboxResolution::Confined(backend) => {
            assert_eq!(backend.name(), "seatbelt");
        }
        SandboxResolution::Unavailable { .. } => {
            // On unsupported platforms, this is expected
        }
    }
}

#[tokio::test]
async fn end_to_end_simple() {
    let llm = Arc::new(ScriptedLlm::new(vec![LlmResponse {
        content: "The answer is 42.".into(),
        finish_reason: "stop".into(),
        usage: None,
        tool_calls: None,
    }]));
    let mut runner = AgentRunner::new(llm, "test-model".into());

    let response = runner.run_turn("What is the meaning of life?".into()).await;
    assert!(response.is_ok());
    assert_eq!(response.unwrap(), "The answer is 42.");

    let log = runner.session_log();
    assert_eq!(log.len(), 7); // TurnStart + UserMessage + StepStart + Chunk + Assistant + StepEnd + TurnEnd
}

#[tokio::test]
async fn end_to_end_with_tool() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        LlmResponse {
            content: String::new(),
            finish_reason: "tool_calls".into(),
            usage: None,
            tool_calls: Some(vec![LlmToolCall {
                id: "call_1".into(),
                call_type: "function".into(),
                function: FunctionCall {
                    name: "calc".into(),
                    arguments: r#"{"expr": "2+3"}"#.into(),
                },
            }]),
        },
        LlmResponse {
            content: "The result is 5.".into(),
            finish_reason: "stop".into(),
            usage: None,
            tool_calls: None,
        },
    ]));
    let mut runner = AgentRunner::new(llm, "test-model".into());
    runner.register_tool(Arc::new(CalcTool));

    let response = runner.run_turn("What is 2+3?".into()).await;
    assert!(response.is_ok());
    assert_eq!(response.unwrap(), "The result is 5.");

    let log = runner.session_log();
    // TurnStart + UserMessage + StepStart(tool) + Chunk(tool) + AssistantMessage(tool) + ToolCall + ToolResult
    // + StepEnd(tool) + StepStart(final) + Chunk(final) + AssistantMessage(final) + StepEnd(final) + TurnEnd
    assert_eq!(log.len(), 13);
}
