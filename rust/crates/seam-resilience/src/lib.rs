//! Resilience — circuit breaker and fault tolerance patterns.

pub mod circuit_breaker;
pub mod retry;

pub use circuit_breaker::{CircuitBreaker, CircuitConfig, CircuitState};
pub use retry::{RetryConfig, RetryPolicy};
