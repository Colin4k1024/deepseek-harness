//! Distributed tracing — trace context propagation and span management.

pub mod context;
pub mod propagation;
pub mod span;

pub use context::{SpanId, TraceContext, TraceId};
pub use propagation::{PropagationError, TracePropagator};
pub use span::{SpanBuilder, SpanEvent, SpanStatus};
