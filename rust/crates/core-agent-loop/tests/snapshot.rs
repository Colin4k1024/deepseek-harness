use std::sync::Arc;

use async_trait::async_trait;
use futures::stream;
use std::pin::Pin;

use dsh_core_agent_loop::{AgentRunner, TurnState};
use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};
use dsh_llm_adapter::{
    FunctionCall, LlmAdapter, LlmChunk, LlmError, LlmRequest, LlmResponse, ToolCall as LlmToolCall,
};

/// A mock LLM that returns pre-scripted responses in sequence.
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

/// A simple echo tool for testing.
struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "echo".into(),
            description: "Echoes the input text".into(),
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

#[tokio::test]
async fn simple_response_no_tools() {
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
}

#[tokio::test]
async fn tool_call_then_final_answer() {
    // First response: structured tool call. Second response: final answer.
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
                    arguments: r#"{"text": "test input"}"#.into(),
                },
            }]),
        },
        LlmResponse {
            content: "Tool said: echo: test input".into(),
            finish_reason: "stop".into(),
            usage: None,
            tool_calls: None,
        },
    ]));
    let mut runner = AgentRunner::new(llm, "test-model".into());
    runner.register_tool(Arc::new(EchoTool));

    let response = runner
        .run_turn("Please echo 'test input'".into())
        .await
        .unwrap();
    assert_eq!(response, "Tool said: echo: test input");
    assert_eq!(*runner.state(), TurnState::Closed);
}
