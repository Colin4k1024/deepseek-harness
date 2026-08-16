//! OpenAI LLM provider.
//!
//! Supports the OpenAI API and any OpenAI-compatible endpoint
//! (Groq, OpenRouter, Together, vLLM, etc.).

pub mod provider;

pub use provider::OpenAiAdapter;
