//! L0: Effect & Context core.
//!
//! Provides `Context` (type-erased service container), `ScopeGuard` (RAII undo),
//! `ScopedContext` (layered scope), `EventBus` (typed event dispatch with waterfall semantics),
//! and `ProviderRegistry` (hot-replace notification via `tokio::sync::watch`).

pub mod context;
pub mod event;
pub mod provider;
pub mod scope;
pub mod undo;

pub use context::Context;
pub use event::{EventBus, Subscription};
pub use provider::ProviderRegistry;
pub use scope::{ScopeGuard, ScopedContext};
