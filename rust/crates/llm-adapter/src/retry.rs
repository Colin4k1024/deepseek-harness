//! Retry wrapper for LLM adapters.

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crate::{LlmAdapter, LlmChunk, LlmError, LlmRequest, LlmResponse};

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
        }
    }
}

/// Wrapper that adds retry logic to an LLM adapter.
pub struct RetryingAdapter {
    inner: Arc<dyn LlmAdapter>,
    config: RetryConfig,
}

impl RetryingAdapter {
    pub fn new(inner: Arc<dyn LlmAdapter>, config: RetryConfig) -> Self {
        Self { inner, config }
    }

    async fn retry<F, Fut, T>(&self, f: F) -> Result<T, LlmError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, LlmError>>,
    {
        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if attempt < self.config.max_retries {
                        let delay = std::cmp::min(
                            self.config.base_delay * 2u32.pow(attempt),
                            self.config.max_delay,
                        );
                        tokio::time::sleep(delay).await;
                        last_error = Some(e);
                    } else {
                        return Err(last_error.unwrap_or(e));
                    }
                }
            }
        }
        unreachable!()
    }
}

#[async_trait]
impl LlmAdapter for RetryingAdapter {
    async fn stream(
        &self,
        request: LlmRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<LlmChunk, LlmError>> + Send>>, LlmError> {
        self.retry(|| self.inner.stream(request.clone())).await
    }

    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        self.retry(|| self.inner.complete(request.clone())).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
    }
}
