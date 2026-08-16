//! Incremental computation — change propagation and dirty tracking.

pub mod computation;
pub use computation::{ComputationGraph, ComputationNode, NodeId};
