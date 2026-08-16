//! Plugin registry — manages plugin lifecycle.

use std::collections::HashMap;
use tokio::sync::RwLock;

use crate::plugin::{Plugin, PluginError, PluginMetadata, PluginState};

/// Plugin registry — stores and manages plugins.
pub struct PluginRegistry {
    plugins: RwLock<HashMap<String, Box<dyn Plugin>>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
        }
    }

    /// Register a plugin.
    pub async fn register(&self, mut plugin: Box<dyn Plugin>) -> Result<(), PluginError> {
        let meta = plugin.metadata();

        // Check dependencies
        {
            let plugins = self.plugins.read().await;
            for dep in &meta.dependencies {
                if !plugins.contains_key(dep) {
                    return Err(PluginError::DependencyNotFound(dep.clone()));
                }
            }
        }

        // Initialize
        plugin.initialize(serde_json::json!({})).await?;

        let mut plugins = self.plugins.write().await;
        plugins.insert(meta.id.clone(), plugin);
        Ok(())
    }

    /// Activate a plugin by ID.
    pub async fn activate(&self, plugin_id: &str) -> Result<(), PluginError> {
        let mut plugins = self.plugins.write().await;
        if let Some(plugin) = plugins.get_mut(plugin_id) {
            plugin.activate().await
        } else {
            Err(PluginError::RuntimeError(format!(
                "plugin not found: {plugin_id}"
            )))
        }
    }

    /// Deactivate a plugin by ID.
    pub async fn deactivate(&self, plugin_id: &str) -> Result<(), PluginError> {
        let mut plugins = self.plugins.write().await;
        if let Some(plugin) = plugins.get_mut(plugin_id) {
            plugin.deactivate().await
        } else {
            Err(PluginError::RuntimeError(format!(
                "plugin not found: {plugin_id}"
            )))
        }
    }

    /// Get a plugin's metadata.
    pub async fn metadata(&self, plugin_id: &str) -> Option<PluginMetadata> {
        let plugins = self.plugins.read().await;
        plugins.get(plugin_id).map(|p| p.metadata())
    }

    /// Get a plugin's state.
    pub async fn state(&self, plugin_id: &str) -> Option<PluginState> {
        let plugins = self.plugins.read().await;
        plugins.get(plugin_id).map(|p| p.state())
    }

    /// List all registered plugin IDs.
    pub async fn list(&self) -> Vec<String> {
        let plugins = self.plugins.read().await;
        plugins.keys().cloned().collect()
    }

    /// Get all tools from all active plugins.
    pub async fn all_tools(&self) -> Vec<Box<dyn dsh_core_tools::Tool>> {
        let plugins = self.plugins.read().await;
        let mut tools = Vec::new();
        for plugin in plugins.values() {
            if plugin.state() == PluginState::Active {
                tools.extend(plugin.tools());
            }
        }
        tools
    }

    /// Send an event to a specific plugin.
    pub async fn emit(
        &self,
        plugin_id: &str,
        event: &str,
        data: serde_json::Value,
    ) -> Result<(), PluginError> {
        let mut plugins = self.plugins.write().await;
        if let Some(plugin) = plugins.get_mut(plugin_id) {
            plugin.handle_event(event, data).await
        } else {
            Err(PluginError::RuntimeError(format!(
                "plugin not found: {plugin_id}"
            )))
        }
    }

    /// Number of registered plugins.
    pub async fn len(&self) -> usize {
        self.plugins.read().await.len()
    }

    /// Whether the registry is empty.
    pub async fn is_empty(&self) -> bool {
        self.plugins.read().await.is_empty()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct TestPlugin {
        id: String,
        state: PluginState,
    }

    #[async_trait]
    impl Plugin for TestPlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata {
                id: self.id.clone(),
                name: format!("Test Plugin {}", self.id),
                version: "0.1.0".into(),
                description: "A test plugin".into(),
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
    async fn register_and_activate() {
        let registry = PluginRegistry::new();
        let plugin = Box::new(TestPlugin {
            id: "test-1".into(),
            state: PluginState::Registered,
        });

        registry.register(plugin).await.unwrap();
        assert_eq!(registry.len().await, 1);

        let state = registry.state("test-1").await.unwrap();
        assert_eq!(state, PluginState::Ready);

        registry.activate("test-1").await.unwrap();
        let state = registry.state("test-1").await.unwrap();
        assert_eq!(state, PluginState::Active);
    }

    #[tokio::test]
    async fn deactivate_plugin() {
        let registry = PluginRegistry::new();
        let plugin = Box::new(TestPlugin {
            id: "test-1".into(),
            state: PluginState::Registered,
        });

        registry.register(plugin).await.unwrap();
        registry.activate("test-1").await.unwrap();
        registry.deactivate("test-1").await.unwrap();

        let state = registry.state("test-1").await.unwrap();
        assert_eq!(state, PluginState::Inactive);
    }

    #[tokio::test]
    async fn plugin_not_found() {
        let registry = PluginRegistry::new();
        assert!(registry.activate("nonexistent").await.is_err());
    }

    #[tokio::test]
    async fn list_plugins() {
        let registry = PluginRegistry::new();
        registry
            .register(Box::new(TestPlugin {
                id: "a".into(),
                state: PluginState::Registered,
            }))
            .await
            .unwrap();
        registry
            .register(Box::new(TestPlugin {
                id: "b".into(),
                state: PluginState::Registered,
            }))
            .await
            .unwrap();

        let mut ids = registry.list().await;
        ids.sort();
        assert_eq!(ids, vec!["a", "b"]);
    }
}
