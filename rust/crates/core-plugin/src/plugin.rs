//! Plugin trait and metadata.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Plugin lifecycle states.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginState {
    /// Plugin is registered but not initialized.
    Registered,
    /// Plugin is initialized and ready.
    Ready,
    /// Plugin is actively running.
    Active,
    /// Plugin is paused.
    Paused,
    /// Plugin has been deactivated.
    Inactive,
    /// Plugin encountered an error.
    Error(String),
}

/// Plugin metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Unique plugin ID.
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Plugin version.
    pub version: String,

    /// Plugin description.
    pub description: String,

    /// Plugin author.
    pub author: String,

    /// Dependencies (other plugin IDs).
    pub dependencies: Vec<String>,

    /// Configuration schema (JSON Schema).
    pub config_schema: serde_json::Value,

    /// Custom metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Plugin errors.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("plugin initialization failed: {0}")]
    InitFailed(String),
    #[error("plugin dependency not found: {0}")]
    DependencyNotFound(String),
    #[error("plugin configuration error: {0}")]
    ConfigError(String),
    #[error("plugin runtime error: {0}")]
    RuntimeError(String),
}

/// Trait for plugins.
///
/// Plugins provide tools, services, and custom behavior to the agent framework.
/// They support lifecycle hooks for initialization, activation, and cleanup.
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Return plugin metadata.
    fn metadata(&self) -> PluginMetadata;

    /// Initialize the plugin with configuration.
    async fn initialize(&mut self, config: serde_json::Value) -> Result<(), PluginError>;

    /// Activate the plugin (called after all plugins are initialized).
    async fn activate(&mut self) -> Result<(), PluginError>;

    /// Deactivate the plugin (called before removal or shutdown).
    async fn deactivate(&mut self) -> Result<(), PluginError>;

    /// Return tools provided by this plugin.
    fn tools(&self) -> Vec<Box<dyn dsh_core_tools::Tool>> {
        Vec::new()
    }

    /// Return the current state.
    fn state(&self) -> PluginState;

    /// Handle a custom message/event.
    async fn handle_event(
        &mut self,
        _event: &str,
        _data: serde_json::Value,
    ) -> Result<(), PluginError> {
        Ok(())
    }
}
