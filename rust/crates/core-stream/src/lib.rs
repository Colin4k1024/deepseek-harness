//! Stream processing — real-time data stream operations.

pub mod pipeline;
pub use pipeline::{Processor, StreamEvent, StreamPipeline};
