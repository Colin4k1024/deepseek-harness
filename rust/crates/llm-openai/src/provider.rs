//! OpenAI LLM provider with SSE streaming support.
//!
//! Implements `LlmAdapter` for the OpenAI API (`/v1/chat/completions`).
//! Compatible with any OpenAI-API-compatible endpoint (OpenRouter, Groq, Together, etc.).

use async_trait::async_trait;
use futures::StreamExt;
use futures::stream::Stream;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::pin::Pin;

use dsh_llm_adapter::{
    FunctionCall, LlmAdapter, LlmChunk, LlmError, LlmRequest, LlmResponse, Message, Role,
    ToolCall as LlmToolCall, ToolDefinition,
};

/// OpenAI API adapter with SSE streaming support.
pub struct OpenAiAdapter {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl OpenAiAdapter {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.openai.com/v1".into(),
            client: reqwest::Client::new(),
        }
    }

    /// Override the base URL for OpenAI-compatible providers.
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
impl LlmAdapter for OpenAiAdapter {
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

        let byte_stream = response.bytes_stream();
        let chunk_stream = byte_stream
            .scan(String::new(), |buffer, result| {
                let chunks = match result {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                        let mut chunks = Vec::new();

                        // Take ownership of buffer content for processing
                        let content = std::mem::take(buffer);
                        let mut lines: Vec<&str> = content.split('\n').collect();

                        // Last element might be partial (no trailing \n)
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

                        // Put back partial line
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
        let line = r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"/tmp/test.txt\"}"}}]},"finish_reason":null}]}"#;
        let result = parse_sse_line(line).unwrap().unwrap();
        assert!(result.delta.is_empty());
        let tool_calls = result.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_1");
        assert_eq!(tool_calls[0].function.name, "read_file");
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
        assert!(api_msgs[0].content.is_none());
        assert!(api_msgs[0].tool_calls.is_some());
    }

    #[test]
    fn to_api_messages_tool_result() {
        let messages = vec![Message {
            role: Role::Tool,
            content: "output".into(),
            tool_calls: None,
            tool_call_id: Some("call_1".into()),
        }];

        let api_msgs = to_api_messages(&messages);
        assert_eq!(api_msgs.len(), 1);
        assert_eq!(api_msgs[0].role, "tool");
        assert_eq!(api_msgs[0].tool_call_id, Some("call_1".into()));
    }
}
