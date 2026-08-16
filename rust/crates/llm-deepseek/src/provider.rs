use async_trait::async_trait;
use futures::Stream;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::pin::Pin;

use dsh_llm_adapter::{
    FunctionCall, LlmAdapter, LlmChunk, LlmError, LlmRequest, LlmResponse, Message, Role,
    ToolCall as LlmToolCall, ToolDefinition,
};

/// DeepSeek API adapter with SSE streaming support.
pub struct DeepSeekAdapter {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl DeepSeekAdapter {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.deepseek.com".into(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key)).unwrap(),
        );
        headers
    }
}

#[derive(Serialize)]
struct ApiRequest {
    model: String,
    messages: Vec<ApiMessage>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDefinition>>,
}

#[derive(Serialize, Deserialize)]
struct ApiMessage {
    role: String,
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ApiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct ApiToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: ApiFunctionCall,
}

#[derive(Serialize, Deserialize, Clone)]
struct ApiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct StreamDelta {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<StreamToolCall>>,
}

#[derive(Deserialize)]
struct StreamToolCall {
    index: usize,
    id: Option<String>,
    #[serde(rename = "type")]
    call_type: Option<String>,
    function: Option<StreamFunctionCall>,
}

#[derive(Deserialize)]
struct StreamFunctionCall {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Deserialize)]
struct CompletionResponse {
    choices: Vec<CompletionChoice>,
}

#[derive(Deserialize)]
struct CompletionChoice {
    message: CompletionMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct CompletionMessage {
    #[allow(dead_code)]
    role: String,
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ApiToolCall>>,
}

fn to_api_messages(messages: &[Message]) -> Vec<ApiMessage> {
    messages
        .iter()
        .map(|m| {
            let tool_calls = m.tool_calls.as_ref().map(|calls| {
                calls
                    .iter()
                    .map(|c| ApiToolCall {
                        id: c.id.clone(),
                        call_type: c.call_type.clone(),
                        function: ApiFunctionCall {
                            name: c.function.name.clone(),
                            arguments: c.function.arguments.clone(),
                        },
                    })
                    .collect()
            });

            ApiMessage {
                role: match m.role {
                    Role::System => "system".into(),
                    Role::User => "user".into(),
                    Role::Assistant => "assistant".into(),
                    Role::Tool => "tool".into(),
                },
                content: if m.content.is_empty() && m.tool_calls.is_some() {
                    None
                } else {
                    Some(m.content.clone())
                },
                tool_calls,
                tool_call_id: m.tool_call_id.clone(),
            }
        })
        .collect()
}

/// Parse an SSE line into an LlmChunk, if applicable.
fn parse_sse_line(line: &str) -> Option<Result<LlmChunk, LlmError>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return None;
    }
    if line == "data: [DONE]" {
        return Some(Ok(LlmChunk {
            delta: String::new(),
            finish_reason: Some("stop".into()),
            tool_calls: None,
        }));
    }
    let data = line.strip_prefix("data: ")?;
    match serde_json::from_str::<StreamChunk>(data) {
        Ok(chunk) => {
            let choice = chunk.choices.first()?;
            let delta = choice.delta.content.clone().unwrap_or_default();
            let tool_calls = choice.delta.tool_calls.as_ref().map(|calls| {
                calls
                    .iter()
                    .filter_map(|c| {
                        let func = c.function.as_ref()?;
                        Some(LlmToolCall {
                            id: c.id.clone().unwrap_or_else(|| format!("call_{}", c.index)),
                            call_type: c.call_type.clone().unwrap_or_else(|| "function".into()),
                            function: FunctionCall {
                                name: func.name.clone().unwrap_or_default(),
                                arguments: func.arguments.clone().unwrap_or_default(),
                            },
                        })
                    })
                    .collect()
            });

            if delta.is_empty() && choice.finish_reason.is_none() && tool_calls.is_none() {
                return None;
            }
            Some(Ok(LlmChunk {
                delta,
                finish_reason: choice.finish_reason.clone(),
                tool_calls,
            }))
        }
        Err(e) => Some(Err(LlmError::Api(format!("SSE parse error: {e}")))),
    }
}

#[async_trait]
impl LlmAdapter for DeepSeekAdapter {
    async fn stream(
        &self,
        request: LlmRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<LlmChunk, LlmError>> + Send>>, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);
        let body = ApiRequest {
            model: request.model,
            messages: to_api_messages(&request.messages),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            stream: true,
            tools: request.tools,
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

        // Collect all bytes first, then process
        let bytes = response
            .bytes()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;
        let text = String::from_utf8_lossy(&bytes);
        let chunks: Vec<Result<LlmChunk, LlmError>> =
            text.lines().filter_map(parse_sse_line).collect();
        let chunk_stream = futures::stream::iter(chunks);

        Ok(Box::pin(chunk_stream))
    }

    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);
        let body = ApiRequest {
            model: request.model,
            messages: to_api_messages(&request.messages),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            stream: false,
            tools: request.tools,
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

        let choice = api_resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| LlmError::Api("no choices in response".into()))?;

        let tool_calls = choice.message.tool_calls.map(|calls| {
            calls
                .into_iter()
                .map(|c| LlmToolCall {
                    id: c.id,
                    call_type: c.call_type,
                    function: FunctionCall {
                        name: c.function.name,
                        arguments: c.function.arguments,
                    },
                })
                .collect()
        });

        Ok(LlmResponse {
            content: choice.message.content.unwrap_or_default(),
            finish_reason: choice.finish_reason.unwrap_or_default(),
            usage: None,
            tool_calls,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sse_text_chunk() {
        let line = r#"data: {"choices":[{"delta":{"content":"hello"},"finish_reason":null}]}"#;
        let result = parse_sse_line(line).unwrap().unwrap();
        assert_eq!(result.delta, "hello");
        assert!(result.tool_calls.is_none());
    }

    #[test]
    fn parse_sse_tool_call_chunk() {
        let line = r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"bash","arguments":"{\"command\":\"ls\"}"}}]},"finish_reason":null}]}"#;
        let result = parse_sse_line(line).unwrap().unwrap();
        assert!(result.delta.is_empty());
        let tool_calls = result.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_1");
        assert_eq!(tool_calls[0].function.name, "bash");
        assert_eq!(tool_calls[0].function.arguments, "{\"command\":\"ls\"}");
    }

    #[test]
    fn parse_sse_done() {
        let result = parse_sse_line("data: [DONE]").unwrap().unwrap();
        assert_eq!(result.finish_reason, Some("stop".into()));
    }

    #[test]
    fn parse_sse_empty_line() {
        assert!(parse_sse_line("").is_none());
        assert!(parse_sse_line(": comment").is_none());
    }

    #[test]
    fn to_api_messages_with_tool_calls() {
        use dsh_llm_adapter::{FunctionCall, ToolCall as LlmToolCall};

        let messages = vec![Message {
            role: Role::Assistant,
            content: String::new(),
            tool_calls: Some(vec![LlmToolCall {
                id: "call_1".into(),
                call_type: "function".into(),
                function: FunctionCall {
                    name: "bash".into(),
                    arguments: "{\"command\":\"ls\"}".into(),
                },
            }]),
            tool_call_id: None,
        }];

        let api_msgs = to_api_messages(&messages);
        assert_eq!(api_msgs.len(), 1);
        assert!(api_msgs[0].content.is_none()); // empty content with tool_calls
        assert!(api_msgs[0].tool_calls.is_some());
        assert_eq!(api_msgs[0].tool_calls.as_ref().unwrap()[0].function.name, "bash");
    }

    #[test]
    fn to_api_messages_tool_result() {
        let messages = vec![Message {
            role: Role::Tool,
            content: "file1.txt\nfile2.txt".into(),
            tool_calls: None,
            tool_call_id: Some("call_1".into()),
        }];

        let api_msgs = to_api_messages(&messages);
        assert_eq!(api_msgs.len(), 1);
        assert_eq!(api_msgs[0].role, "tool");
        assert_eq!(api_msgs[0].tool_call_id, Some("call_1".into()));
    }
}
