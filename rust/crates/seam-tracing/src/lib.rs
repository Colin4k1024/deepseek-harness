//! Distributed tracing — W3C trace context, log aggregation, and structured logging.

pub mod aggregator;
pub mod context;
pub mod log_entry;
pub mod propagation;
pub mod span;

pub use aggregator::LogAggregator;
pub use context::TraceContext;
pub use log_entry::{LogEntry, LogLevel};
pub use propagation::TracePropagator;
pub use span::{SpanBuilder, SpanStatus};
