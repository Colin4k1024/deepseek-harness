//! Anthropic Claude LLM provider with SSE streaming support.
//!
//! Implements `LlmAdapter` for the Anthropic Messages API (`/v1/messages`).

use async_trait::async_trait;
use futures::StreamExt;
use futures::stream::Stream;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::pin::Pin;

use dsh_llm_adapter::{
    FunctionCall, LlmAdapter, LlmChunk, LlmError, LlmRequest, LlmResponse, Message, Role,
    ToolCall as LlmToolCall, ToolDefinition,
};

/// Anthropic Claude API adapter with SSE streaming support.
pub struct AnthropicAdapter {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
    anthropic_version: String,
}

impl AnthropicAdapter {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.anthropic.com".into(),
            client: reqwest::Client::new(),
            anthropic_version: "2023-06-01".into(),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert("x-api-key", HeaderValue::from_str(&self.api_key).unwrap());
        headers.insert(
            "anthropic-version",
            HeaderValue::from_str(&self.anthropic_version).unwrap(),
        );
        headers
    }
}

#[derive(Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
}

#[derive(Serialize, Deserialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct ApiMessage {
    role: String,
    content: serde_json::Value, // Can be string or array of content blocks
}

/// Anthropic SSE events have different types.
#[derive(Deserialize)]
#[serde(tag = "type")]
enum StreamEvent {
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { delta: ContentDelta },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "message_start")]
    #[allow(dead_code)]
    MessageStart { message: MessageStartData },
    #[serde(rename = "message_delta")]
    MessageDelta { delta: MessageDeltaData },
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        content_block: Option<ContentBlockStartData>,
    },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop,
    #[serde(rename = "error")]
    Error { error: ErrorData },
}

#[derive(Deserialize)]
struct ContentBlockStartData {
    #[serde(rename = "type")]
    block_type: Option<String>,
    id: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct ContentDelta {
    #[serde(rename = "type")]
    delta_type: String,
    text: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    input_json_delta: Option<InputJsonDelta>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct InputJsonDelta {
    partial_json: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct MessageStartData {
    id: String,
}

#[derive(Deserialize)]
struct MessageDeltaData {
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct ErrorData {
    message: String,
}

#[derive(Deserialize)]
struct CompletionResponse {
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
    id: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

/// Convert dsh messages to Anthropic format, extracting system messages separately.
fn to_api_messages(messages: &[Message]) -> (Option<String>, Vec<ApiMessage>) {
    let mut system = None;
    let mut api_messages = Vec::new();

    for msg in messages {
        match msg.role {
            Role::System => {
                system = Some(msg.content.clone());
            }
            Role::User => api_messages.push(ApiMessage {
                role: "user".into(),
                content: serde_json::json!(msg.content),
            }),
            Role::Assistant => {
                if let Some(ref tool_calls) = msg.tool_calls {
                    // Assistant message with tool use
                    let mut blocks = vec![serde_json::json!({
                        "type": "text",
                        "text": msg.content
                    })];
                    for tc in tool_calls {
                        let args: serde_json::Value =
                            serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                        blocks.push(serde_json::json!({
                            "type": "tool_use",
                            "id": tc.id,
                            "name": tc.function.name,
                            "input": args
                        }));
                    }
                    api_messages.push(ApiMessage {
                        role: "assistant".into(),
                        content: serde_json::json!(blocks),
                    });
                } else {
                    api_messages.push(ApiMessage {
                        role: "assistant".into(),
                        content: serde_json::json!(msg.content),
                    });
                }
            }
            Role::Tool => {
                // Tool result message
                let tool_call_id = msg.tool_call_id.clone().unwrap_or_default();
                api_messages.push(ApiMessage {
                    role: "user".into(),
                    content: serde_json::json!([{
                        "type": "tool_result",
                        "tool_use_id": tool_call_id,
                        "content": msg.content
                    }]),
                });
            }
        }
    }

    (system, api_messages)
}

/// Convert dsh tool definitions to Anthropic format.
fn to_anthropic_tools(tools: &[ToolDefinition]) -> Vec<AnthropicTool> {
    tools
        .iter()
        .map(|t| AnthropicTool {
            name: t.function.name.clone(),
            description: t.function.description.clone(),
            input_schema: t.function.parameters.clone(),
        })
        .collect()
}

fn parse_sse_line(line: &str) -> Option<Result<LlmChunk, LlmError>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return None;
    }

    let data = line.strip_prefix("data: ")?;

    match serde_json::from_str::<StreamEvent>(data) {
        Ok(StreamEvent::ContentBlockDelta { delta }) => {
            if delta.delta_type == "text_delta" {
                let text = delta.text.unwrap_or_default();
                if !text.is_empty() {
                    return Some(Ok(LlmChunk {
                        delta: text,
                        finish_reason: None,
                        tool_calls: None,
                    }));
                }
            }
            // tool use input_json_delta - we collect these in the agent loop
            None
        }
        Ok(StreamEvent::ContentBlockStart { content_block }) => {
            if let Some(block) = content_block
                && block.block_type.as_deref() == Some("tool_use")
            {
                // Tool use block started - emit a tool call
                let tool_call = LlmToolCall {
                    id: block.id.unwrap_or_default(),
                    call_type: "function".into(),
                    function: FunctionCall {
                        name: block.name.unwrap_or_default(),
                        arguments: String::new(), // Will be filled by input_json_delta
                    },
                };
                return Some(Ok(LlmChunk {
                    delta: String::new(),
                    finish_reason: None,
                    tool_calls: Some(vec![tool_call]),
                }));
            }
            None
        }
        Ok(StreamEvent::MessageDelta { delta }) => Some(Ok(LlmChunk {
            delta: String::new(),
            finish_reason: delta.stop_reason,
            tool_calls: None,
        })),
        Ok(StreamEvent::MessageStop) => Some(Ok(LlmChunk {
            delta: String::new(),
            finish_reason: Some("stop".into()),
            tool_calls: None,
        })),
        Ok(StreamEvent::Error { error }) => Some(Err(LlmError::Api(format!(
            "Anthropic error: {}",
            error.message
        )))),
        Ok(_) => None,
        Err(e) => Some(Err(LlmError::Api(format!("SSE parse error: {e}")))),
    }
}

#[async_trait]
impl LlmAdapter for AnthropicAdapter {
    async fn stream(
        &self,
        request: LlmRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<LlmChunk, LlmError>> + Send>>, LlmError> {
        let url = format!("{}/v1/messages", self.base_url);
        let (system, messages) = to_api_messages(&request.messages);
        let tools = request.tools.map(|t| to_anthropic_tools(&t));

        let body = ApiRequest {
            model: request.model,
            max_tokens: request.max_tokens.unwrap_or(4096),
            messages,
            system,
            temperature: request.temperature,
            stream: true,
            tools,
        };

        let response = self
            .client
            .post(&url)
            .headers(self.build_headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable>".into());
            return Err(LlmError::Api(format!("{status}: {body}")));
        }

        let byte_stream = response.bytes_stream();
        let chunk_stream = byte_stream
            .scan(String::new(), |buffer, result| {
                let chunks = match result {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                        let mut chunks = Vec::new();

                        let content = std::mem::take(buffer);
                        let mut lines: Vec<&str> = content.split('\n').collect();
                        let partial = if content.ends_with('\n') {
                            None
                        } else {
                            lines.pop()
                        };

                        for line in lines {
                            let trimmed = line.trim_end_matches('\r');
                            if trimmed.is_empty() || trimmed.starts_with(':') {
                                continue;
                            }
                            if let Some(chunk) = parse_sse_line(trimmed) {
                                chunks.push(chunk);
                            }
                        }

                        if let Some(p) = partial {
                            buffer.push_str(p);
                        }

                        Ok(chunks)
                    }
                    Err(e) => Err(LlmError::Network(e.to_string())),
                };
                futures::future::ready(Some(chunks))
            })
            .flat_map(|result| match result {
                Ok(chunks) => futures::stream::iter(chunks).left_stream(),
                Err(e) => futures::stream::iter(vec![Err(e)]).right_stream(),
            });

        Ok(Box::pin(chunk_stream))
    }

    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let url = format!("{}/v1/messages", self.base_url);
        let (system, messages) = to_api_messages(&request.messages);
        let tools = request.tools.map(|t| to_anthropic_tools(&t));

        let body = ApiRequest {
            model: request.model,
            max_tokens: request.max_tokens.unwrap_or(4096),
            messages,
            system,
            temperature: request.temperature,
            stream: false,
            tools,
        };

        let response = self
            .client
            .post(&url)
            .headers(self.build_headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable>".into());
            return Err(LlmError::Api(format!("{status}: {body}")));
        }

        let api_resp: CompletionResponse = response
            .json()
            .await
            .map_err(|e| LlmError::Api(format!("response parse error: {e}")))?;

        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for block in &api_resp.content {
            match block.block_type.as_str() {
                "text" => {
                    if let Some(text) = &block.text {
                        text_parts.push(text.clone());
                    }
                }
                "tool_use" => {
                    let args = block
                        .input
                        .as_ref()
                        .map(|v| serde_json::to_string(v).unwrap_or_default())
                        .unwrap_or_default();
                    tool_calls.push(LlmToolCall {
                        id: block.id.clone().unwrap_or_default(),
                        call_type: "function".into(),
                        function: FunctionCall {
                            name: block.name.clone().unwrap_or_default(),
                            arguments: args,
                        },
                    });
                }
                _ => {}
            }
        }

        Ok(LlmResponse {
            content: text_parts.join("\n"),
            finish_reason: api_resp.stop_reason.unwrap_or_default(),
            usage: None,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sse_text_delta() {
        let line =
            r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"hello"}}"#;
        let result = parse_sse_line(line).unwrap().unwrap();
        assert_eq!(result.delta, "hello");
        assert!(result.tool_calls.is_none());
    }

    #[test]
    fn parse_sse_tool_use_start() {
        let line = r#"data: {"type":"content_block_start","content_block":{"type":"tool_use","id":"toolu_1","name":"bash"}}"#;
        let result = parse_sse_line(line).unwrap().unwrap();
        assert!(result.delta.is_empty());
        let tool_calls = result.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "toolu_1");
        assert_eq!(tool_calls[0].function.name, "bash");
    }

    #[test]
    fn parse_sse_message_stop() {
        let line = r#"data: {"type":"message_stop"}"#;
        let result = parse_sse_line(line).unwrap().unwrap();
        assert_eq!(result.finish_reason, Some("stop".into()));
    }

    #[test]
    fn parse_sse_empty_line() {
        assert!(parse_sse_line("").is_none());
        assert!(parse_sse_line(": comment").is_none());
    }

    #[test]
    fn to_api_messages_system() {
        let messages = vec![Message {
            role: Role::System,
            content: "You are a helpful assistant.".into(),
            tool_calls: None,
            tool_call_id: None,
        }];
        let (system, api_msgs) = to_api_messages(&messages);
        assert_eq!(system, Some("You are a helpful assistant.".into()));
        assert!(api_msgs.is_empty());
    }

    #[test]
    fn to_api_messages_tool_use() {
        use dsh_llm_adapter::{FunctionCall, ToolCall as LlmToolCall};

        let messages = vec![Message {
            role: Role::Assistant,
            content: String::new(),
            tool_calls: Some(vec![LlmToolCall {
                id: "toolu_1".into(),
                call_type: "function".into(),
                function: FunctionCall {
                    name: "bash".into(),
                    arguments: "{\"command\":\"ls\"}".into(),
                },
            }]),
            tool_call_id: None,
        }];

        let (_, api_msgs) = to_api_messages(&messages);
        assert_eq!(api_msgs.len(), 1);
        assert_eq!(api_msgs[0].role, "assistant");
        // Content should be an array with text + tool_use blocks
        let content = &api_msgs[0].content;
        assert!(content.is_array());
        let arr = content.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["type"], "text");
        assert_eq!(arr[1]["type"], "tool_use");
        assert_eq!(arr[1]["name"], "bash");
    }

    #[test]
    fn to_api_messages_tool_result() {
        let messages = vec![Message {
            role: Role::Tool,
            content: "file1.txt\nfile2.txt".into(),
            tool_calls: None,
            tool_call_id: Some("toolu_1".into()),
        }];

        let (_, api_msgs) = to_api_messages(&messages);
        assert_eq!(api_msgs.len(), 1);
        assert_eq!(api_msgs[0].role, "user");
        let content = &api_msgs[0].content;
        assert!(content.is_array());
        let arr = content.as_array().unwrap();
        assert_eq!(arr[0]["type"], "tool_result");
        assert_eq!(arr[0]["tool_use_id"], "toolu_1");
    }

    #[test]
    fn to_anthropic_tools_conversion() {
        use dsh_llm_adapter::FunctionDefinition;

        let tools = vec![ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "bash".into(),
                description: "Execute a bash command".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {"type": "string"}
                    }
                }),
            },
        }];

        let anthropic_tools = to_anthropic_tools(&tools);
        assert_eq!(anthropic_tools.len(), 1);
        assert_eq!(anthropic_tools[0].name, "bash");
        assert_eq!(anthropic_tools[0].description, "Execute a bash command");
    }
}
