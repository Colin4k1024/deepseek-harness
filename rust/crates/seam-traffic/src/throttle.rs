//! Throttle — request throttling and load shedding.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Throttle configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottleConfig {
    /// Maximum requests per window.
    pub max_requests: u64,

    /// Window duration in seconds.
    pub window_secs: u64,

    /// Whether to queue excess requests or reject immediately.
    pub queue_excess: bool,

    /// Maximum queue size (only used if queue_excess is true).
    pub max_queue_size: u32,
}

impl Default for ThrottleConfig {
    fn default() -> Self {
        Self {
            max_requests: 1000,
            window_secs: 60,
            queue_excess: false,
            max_queue_size: 100,
        }
    }
}

/// Throttle — controls request rate with sliding window.
pub struct Throttle {
    config: ThrottleConfig,
    window_start: std::sync::Mutex<Instant>,
    request_count: AtomicU64,
    rejected_count: AtomicU64,
}

impl Throttle {
    /// Create a new throttle.
    pub fn new(config: ThrottleConfig) -> Self {
        Self {
            config,
            window_start: std::sync::Mutex::new(Instant::now()),
            request_count: AtomicU64::new(0),
            rejected_count: AtomicU64::new(0),
        }
    }

    /// Check if a request should be allowed.
    pub fn allow(&self) -> bool {
        let mut start = self.window_start.lock().unwrap();

        // Reset window if elapsed
        if start.elapsed() >= Duration::from_secs(self.config.window_secs) {
            *start = Instant::now();
            self.request_count.store(0, Ordering::Relaxed);
        }

        let count = self.request_count.fetch_add(1, Ordering::Relaxed) + 1;
        if count <= self.config.max_requests {
            true
        } else {
            self.rejected_count.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    /// Get the current request count in the window.
    pub fn current_count(&self) -> u64 {
        self.request_count.load(Ordering::Relaxed)
    }

    /// Get the rejected count.
    pub fn rejected_count(&self) -> u64 {
        self.rejected_count.load(Ordering::Relaxed)
    }

    /// Get the remaining capacity.
    pub fn remaining(&self) -> u64 {
        let count = self.request_count.load(Ordering::Relaxed);
        self.config.max_requests.saturating_sub(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_within_limit() {
        let throttle = Throttle::new(ThrottleConfig {
            max_requests: 5,
            window_secs: 60,
            ..Default::default()
        });

        for _ in 0..5 {
            assert!(throttle.allow());
        }
    }

    #[test]
    fn reject_over_limit() {
        let throttle = Throttle::new(ThrottleConfig {
            max_requests: 3,
            window_secs: 60,
            ..Default::default()
        });

        assert!(throttle.allow());
        assert!(throttle.allow());
        assert!(throttle.allow());
        assert!(!throttle.allow()); // 4th rejected
        assert_eq!(throttle.rejected_count(), 1);
    }

    #[test]
    fn window_reset() {
        let throttle = Throttle::new(ThrottleConfig {
            max_requests: 2,
            window_secs: 1,
            ..Default::default()
        });

        assert!(throttle.allow());
        assert!(throttle.allow());
        assert!(!throttle.allow());

        std::thread::sleep(Duration::from_secs(1) + Duration::from_millis(10));
        assert!(throttle.allow()); // window reset
    }

    #[test]
    fn remaining_capacity() {
        let throttle = Throttle::new(ThrottleConfig {
            max_requests: 10,
            ..Default::default()
        });

        assert_eq!(throttle.remaining(), 10);
        throttle.allow();
        assert_eq!(throttle.remaining(), 9);
    }
}
