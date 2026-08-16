//! Log aggregation — structured log collection and forwarding.

pub mod aggregator;
pub mod entry;

pub use aggregator::{AggregatorConfig, LogAggregator};
pub use entry::{LogEntry, LogLevel};
