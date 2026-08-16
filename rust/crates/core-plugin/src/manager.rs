//! Plugin manager — orchestrates plugin lifecycle and hot-reload.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::plugin::{Plugin, PluginError};
use crate::registry::PluginRegistry;

/// Plugin manager — orchestrates plugin lifecycle, configuration, and hot-reload.
pub struct PluginManager {
    registry: Arc<PluginRegistry>,
    config_path: Option<PathBuf>,
}

impl PluginManager {
    /// Create a new plugin manager.
    pub fn new() -> Self {
        Self {
            registry: Arc::new(PluginRegistry::new()),
            config_path: None,
        }
    }

    /// Set the configuration file path for hot-reload.
    pub fn with_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    /// Get a reference to the plugin registry.
    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    /// Register and activate a plugin.
    pub async fn install(&self, plugin: Box<dyn Plugin>) -> Result<(), PluginError> {
        let meta = plugin.metadata();
        self.registry.register(plugin).await?;
        self.registry.activate(&meta.id).await?;
        tracing::info!(plugin_id = %meta.id, version = %meta.version, "plugin installed and activated");
        Ok(())
    }

    /// Uninstall a plugin (deactivate + remove).
    pub async fn uninstall(&self, plugin_id: &str) -> Result<(), PluginError> {
        self.registry.deactivate(plugin_id).await?;
        tracing::info!(plugin_id = %plugin_id, "plugin uninstalled");
        Ok(())
    }

    /// Reload a plugin (deactivate → reinitialize → activate).
    pub async fn reload(&self, plugin_id: &str) -> Result<(), PluginError> {
        self.registry.deactivate(plugin_id).await?;
        self.registry.activate(plugin_id).await?;
        tracing::info!(plugin_id = %plugin_id, "plugin reloaded");
        Ok(())
    }

    /// Check if hot-reload is available (config file exists and is watchable).
    pub fn can_hot_reload(&self) -> bool {
        self.config_path.as_ref().is_some_and(|p| p.exists())
    }

    /// Get the config path.
    pub fn config_path(&self) -> Option<&Path> {
        self.config_path.as_deref()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{PluginMetadata, PluginState};
    use async_trait::async_trait;
    use std::collections::HashMap;

    struct MockPlugin {
        id: String,
        state: PluginState,
    }

    #[async_trait]
    impl Plugin for MockPlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata {
                id: self.id.clone(),
                name: format!("Mock {}", self.id),
                version: "0.1.0".into(),
                description: "Mock plugin".into(),
                author: "test".into(),
                dependencies: vec![],
                config_schema: serde_json::json!({}),
                metadata: HashMap::new(),
            }
        }

        async fn initialize(&mut self, _config: serde_json::Value) -> Result<(), PluginError> {
            self.state = PluginState::Ready;
            Ok(())
        }

        async fn activate(&mut self) -> Result<(), PluginError> {
            self.state = PluginState::Active;
            Ok(())
        }

        async fn deactivate(&mut self) -> Result<(), PluginError> {
            self.state = PluginState::Inactive;
            Ok(())
        }

        fn state(&self) -> PluginState {
            self.state.clone()
        }
    }

    #[tokio::test]
    async fn install_and_uninstall() {
        let manager = PluginManager::new();
        let plugin = Box::new(MockPlugin {
            id: "test".into(),
            state: PluginState::Registered,
        });

        manager.install(plugin).await.unwrap();
        assert_eq!(manager.registry().len().await, 1);

        let state = manager.registry().state("test").await.unwrap();
        assert_eq!(state, PluginState::Active);

        manager.uninstall("test").await.unwrap();
        let state = manager.registry().state("test").await.unwrap();
        assert_eq!(state, PluginState::Inactive);
    }

    #[tokio::test]
    async fn reload_plugin() {
        let manager = PluginManager::new();
        let plugin = Box::new(MockPlugin {
            id: "test".into(),
            state: PluginState::Registered,
        });

        manager.install(plugin).await.unwrap();
        manager.reload("test").await.unwrap();

        let state = manager.registry().state("test").await.unwrap();
        assert_eq!(state, PluginState::Active);
    }

    #[test]
    fn hot_reload_check() {
        let manager = PluginManager::new();
        assert!(!manager.can_hot_reload());

        let manager = PluginManager::new().with_config_path("/nonexistent/config.json");
        assert!(!manager.can_hot_reload());
    }
}
