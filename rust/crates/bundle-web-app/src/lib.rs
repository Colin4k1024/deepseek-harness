//! L3: Web application bundle — axum-based HTTP surface.
//!
//! Provides `build_router()` to create an axum Router with all agent API endpoints,
//! and `AppState` for shared state (LLM adapter + model name).

pub mod server;

pub use server::{AppState, ChatRequest, ChatResponse, build_router};
