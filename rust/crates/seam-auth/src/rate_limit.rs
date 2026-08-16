//! Rate limiting — token bucket algorithm.
//!
//! Each user/key gets a token bucket that refills at a configured rate.
//! Requests consume tokens; when empty, requests are rejected.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// Rate limiter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum tokens in the bucket (burst capacity).
    pub max_tokens: u32,

    /// Tokens added per second (sustained rate).
    pub refill_rate: f64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_tokens: 60,
            refill_rate: 1.0, // 1 request per second
        }
    }
}

/// Token bucket state for a single user/key.
struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(config: &RateLimitConfig) -> Self {
        Self {
            tokens: config.max_tokens as f64,
            max_tokens: config.max_tokens as f64,
            refill_rate: config.refill_rate,
            last_refill: Instant::now(),
        }
    }

    fn try_consume(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;
    }
}

/// Rate limiter — enforces per-user request rates.
pub struct RateLimiter {
    config: RateLimitConfig,
    buckets: RwLock<HashMap<String, TokenBucket>>,
}

impl RateLimiter {
    /// Create a new rate limiter with the given config.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: RwLock::new(HashMap::new()),
        }
    }

    /// Check if a request is allowed for the given user/key.
    ///
    /// Returns `true` if the request is allowed, `false` if rate limited.
    pub fn check(&self, user_id: &str) -> bool {
        let mut buckets = self.buckets.write().unwrap();
        let bucket = buckets
            .entry(user_id.to_string())
            .or_insert_with(|| TokenBucket::new(&self.config));
        bucket.try_consume()
    }

    /// Get remaining tokens for a user (for response headers).
    pub fn remaining(&self, user_id: &str) -> u32 {
        let mut buckets = self.buckets.write().unwrap();
        if let Some(bucket) = buckets.get_mut(user_id) {
            bucket.refill();
            bucket.tokens as u32
        } else {
            self.config.max_tokens
        }
    }

    /// Reset a user's bucket (admin operation).
    pub fn reset(&self, user_id: &str) {
        self.buckets.write().unwrap().remove(user_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_within_limit() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_tokens: 5,
            refill_rate: 1.0,
        });

        for _ in 0..5 {
            assert!(limiter.check("user1"));
        }
    }

    #[test]
    fn deny_over_limit() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_tokens: 3,
            refill_rate: 0.0, // no refill
        });

        assert!(limiter.check("user1"));
        assert!(limiter.check("user1"));
        assert!(limiter.check("user1"));
        assert!(!limiter.check("user1")); // 4th request denied
    }

    #[test]
    fn per_user_isolation() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_tokens: 1,
            refill_rate: 0.0,
        });

        assert!(limiter.check("user1"));
        assert!(!limiter.check("user1")); // user1 exhausted
        assert!(limiter.check("user2")); // user2 still has tokens
    }

    #[test]
    fn remaining_tokens() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_tokens: 10,
            refill_rate: 1.0,
        });

        assert_eq!(limiter.remaining("user1"), 10); // no requests yet
        limiter.check("user1");
        assert_eq!(limiter.remaining("user1"), 9);
    }
}
