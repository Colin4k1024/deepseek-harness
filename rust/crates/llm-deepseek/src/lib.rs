//! DeepSeek LLM provider implementation.
//!
//! Implements `LlmAdapter` for the DeepSeek API with SSE streaming support.

pub mod provider;

pub use provider::DeepSeekAdapter;
