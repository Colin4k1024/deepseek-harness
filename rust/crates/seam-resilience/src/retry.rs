//! Retry policy — configurable retry with backoff.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Retry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retries.
    pub max_retries: u32,

    /// Base delay between retries.
    pub base_delay: Duration,

    /// Backoff multiplier.
    pub backoff_multiplier: f64,

    /// Maximum delay.
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
            max_delay: Duration::from_secs(30),
        }
    }
}

/// Retry policy — determines delay for each retry attempt.
pub struct RetryPolicy {
    config: RetryConfig,
}

impl RetryPolicy {
    /// Create a new retry policy.
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Get the delay for a given attempt (0-indexed).
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms = self.config.base_delay.as_millis() as f64
            * self.config.backoff_multiplier.powi(attempt as i32);
        let delay = Duration::from_millis(delay_ms as u64);
        delay.min(self.config.max_delay)
    }

    /// Whether a retry should be attempted.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.config.max_retries
    }

    /// Get the maximum retries.
    pub fn max_retries(&self) -> u32 {
        self.config.max_retries
    }
}

/// Execute a closure with retry logic.
pub async fn with_retry<T, E, F, Fut>(policy: &RetryPolicy, mut operation: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut attempt = 0;
    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if policy.should_retry(attempt) {
                    let delay = policy.delay_for_attempt(attempt);
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                } else {
                    return Err(e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_exponential_backoff() {
        let policy = RetryPolicy::new(RetryConfig {
            base_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
            max_delay: Duration::from_secs(10),
            ..Default::default()
        });

        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(400));
    }

    #[test]
    fn delay_capped_at_max() {
        let policy = RetryPolicy::new(RetryConfig {
            base_delay: Duration::from_millis(1000),
            backoff_multiplier: 10.0,
            max_delay: Duration::from_secs(5),
            ..Default::default()
        });

        assert_eq!(policy.delay_for_attempt(5), Duration::from_secs(5));
    }

    #[test]
    fn should_retry() {
        let policy = RetryPolicy::new(RetryConfig {
            max_retries: 3,
            ..Default::default()
        });

        assert!(policy.should_retry(0));
        assert!(policy.should_retry(1));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3));
    }

    #[tokio::test]
    async fn with_retry_success_on_first() {
        let policy = RetryPolicy::new(RetryConfig::default());
        let mut call_count = 0;
        let result = with_retry(&policy, || {
            call_count += 1;
            async { Ok::<_, String>("success") }
        })
        .await;

        assert_eq!(result.unwrap(), "success");
        assert_eq!(call_count, 1);
    }

    #[tokio::test]
    async fn with_retry_success_after_failures() {
        let policy = RetryPolicy::new(RetryConfig {
            max_retries: 3,
            base_delay: Duration::from_millis(1),
            ..Default::default()
        });
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let cc = call_count.clone();

        let result = with_retry(&policy, || {
            let count = cc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            async move {
                if count < 2 {
                    Err("fail")
                } else {
                    Ok("success")
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), "success");
        assert_eq!(call_count.load(std::sync::atomic::Ordering::Relaxed), 3);
    }
}
