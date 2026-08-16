//! Incremental computation — change propagation, dirty tracking, and stream processing.

pub mod computation;
pub mod pipeline;

pub use computation::{ComputationGraph, ComputationNode, NodeId};
pub use pipeline::{Processor, StreamEvent, StreamPipeline};
