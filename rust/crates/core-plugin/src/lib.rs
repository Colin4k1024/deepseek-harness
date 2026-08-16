//! Plugin system — trait-based plugins with lifecycle hooks.
//!
//! Plugins are registered with the context and can provide tools,
//! LLM adapters, and custom services. They support lifecycle hooks
//! for initialization, activation, deactivation, and cleanup.

pub mod manager;
pub mod plugin;
pub mod registry;

pub use manager::PluginManager;
pub use plugin::{Plugin, PluginError, PluginMetadata, PluginState};
pub use registry::PluginRegistry;
