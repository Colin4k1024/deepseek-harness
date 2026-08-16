//! Circuit breaker — fault tolerance pattern.

use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// Circuit breaker configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitConfig {
    /// Number of failures before opening the circuit.
    pub failure_threshold: u32,

    /// Duration to stay open before half-open.
    pub open_duration: Duration,

    /// Number of successful calls in half-open to close.
    pub half_open_threshold: u32,
}

impl Default for CircuitConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            open_duration: Duration::from_secs(30),
            half_open_threshold: 2,
        }
    }
}

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CircuitState {
    /// Normal operation — requests pass through.
    Closed,
    /// Circuit is open — requests are rejected.
    Open,
    /// Testing — limited requests pass through.
    HalfOpen,
}

/// Circuit breaker — prevents cascading failures.
pub struct CircuitBreaker {
    config: CircuitConfig,
    state: RwLock<CircuitState>,
    failure_count: AtomicU32,
    success_count: AtomicU32,
    last_failure: RwLock<Option<Instant>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    pub fn new(config: CircuitConfig) -> Self {
        Self {
            config,
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            last_failure: RwLock::new(None),
        }
    }

    /// Get the current state.
    pub fn state(&self) -> CircuitState {
        let state = self.state.read().unwrap();
        *state
    }

    /// Check if a request is allowed.
    pub fn is_allowed(&self) -> bool {
        let mut state = self.state.write().unwrap();

        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if open duration has elapsed
                let last_failure = self.last_failure.read().unwrap();
                if let Some(last) = *last_failure
                    && last.elapsed() >= self.config.open_duration
                {
                    *state = CircuitState::HalfOpen;
                    self.success_count.store(0, Ordering::Relaxed);
                    return true;
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful call.
    pub fn record_success(&self) {
        let mut state = self.state.write().unwrap();

        match *state {
            CircuitState::Closed => {
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::HalfOpen => {
                let count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                if count >= self.config.half_open_threshold {
                    *state = CircuitState::Closed;
                    self.failure_count.store(0, Ordering::Relaxed);
                    self.success_count.store(0, Ordering::Relaxed);
                }
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed call.
    pub fn record_failure(&self) {
        let mut state = self.state.write().unwrap();

        match *state {
            CircuitState::Closed => {
                let count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                if count >= self.config.failure_threshold {
                    *state = CircuitState::Open;
                    *self.last_failure.write().unwrap() = Some(Instant::now());
                }
            }
            CircuitState::HalfOpen => {
                *state = CircuitState::Open;
                *self.last_failure.write().unwrap() = Some(Instant::now());
                self.success_count.store(0, Ordering::Relaxed);
            }
            CircuitState::Open => {}
        }
    }

    /// Reset the circuit breaker to closed state.
    pub fn reset(&self) {
        let mut state = self.state.write().unwrap();
        *state = CircuitState::Closed;
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        *self.last_failure.write().unwrap() = None;
    }

    /// Get the current failure count.
    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_closed() {
        let cb = CircuitBreaker::new(CircuitConfig::default());
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_allowed());
    }

    #[test]
    fn opens_after_threshold() {
        let cb = CircuitBreaker::new(CircuitConfig {
            failure_threshold: 3,
            ..Default::default()
        });

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.is_allowed());
    }

    #[test]
    fn half_open_after_duration() {
        let cb = CircuitBreaker::new(CircuitConfig {
            failure_threshold: 1,
            open_duration: Duration::from_millis(0),
            ..Default::default()
        });

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        std::thread::sleep(Duration::from_millis(1));
        assert!(cb.is_allowed());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn closes_after_half_open_success() {
        let cb = CircuitBreaker::new(CircuitConfig {
            failure_threshold: 1,
            open_duration: Duration::from_millis(0),
            half_open_threshold: 2,
        });

        cb.record_failure();
        std::thread::sleep(Duration::from_millis(1));
        cb.is_allowed(); // transitions to half-open

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn half_open_failure_reopens() {
        let cb = CircuitBreaker::new(CircuitConfig {
            failure_threshold: 1,
            open_duration: Duration::from_millis(0),
            half_open_threshold: 2,
        });

        cb.record_failure();
        std::thread::sleep(Duration::from_millis(1));
        cb.is_allowed(); // half-open

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn reset() {
        let cb = CircuitBreaker::new(CircuitConfig {
            failure_threshold: 1,
            ..Default::default()
        });

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }
}
