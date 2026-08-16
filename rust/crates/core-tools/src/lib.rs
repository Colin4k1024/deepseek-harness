//! L1: Tool registry and execution pipeline.
//!
//! Defines the `Tool` trait, `ToolSchema`, `ToolResult`, and `ToolPipeline`
//! with pre/post middleware chain semantics.

pub mod pipeline;
pub mod trait_def;

pub use pipeline::ToolPipeline;
pub use trait_def::{Tool, ToolError, ToolResult, ToolSchema};
