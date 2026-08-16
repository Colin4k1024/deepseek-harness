use async_trait::async_trait;
use futures::Stream;
use futures::stream;
use std::pin::Pin;

use dsh_llm_adapter::{LlmAdapter, LlmChunk, LlmError, LlmRequest, LlmResponse};

/// Mock LLM adapter that replays pre-recorded responses from a fixture.
pub struct MockLlm {
    chunks: Vec<LlmChunk>,
}

impl MockLlm {
    /// Create a mock from pre-recorded chunks.
    pub fn new(chunks: Vec<LlmChunk>) -> Self {
        Self { chunks }
    }

    /// Create a mock from a JSONL fixture file (each line is an LlmChunk).
    pub fn from_recorded(chunks: Vec<LlmChunk>) -> Self {
        Self { chunks }
    }
}

#[async_trait]
impl LlmAdapter for MockLlm {
    async fn stream(
        &self,
        _request: LlmRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<LlmChunk, LlmError>> + Send>>, LlmError> {
        let chunks: Vec<Result<LlmChunk, LlmError>> = self.chunks.iter().cloned().map(Ok).collect();
        Ok(Box::pin(stream::iter(chunks)))
    }

    async fn complete(&self, _request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let content: String = self.chunks.iter().map(|c| c.delta.as_str()).collect();
        Ok(LlmResponse {
            content,
            finish_reason: "stop".into(),
            usage: None,
            tool_calls: None,
        })
    }
}
