//! L1: LLM adapter trait and request/response types.
//!
//! Defines the `LlmAdapter` trait for streaming LLM completions,
//! plus `LlmRequest`, `LlmResponse`, and `LlmChunk` types.

pub mod retry;
pub mod types;

pub use retry::RetryingAdapter;
pub use types::{
    FunctionCall, FunctionDefinition, LlmAdapter, LlmChunk, LlmError, LlmRequest, LlmResponse,
    Message, Role, ToolCall, ToolDefinition, Usage,
};
