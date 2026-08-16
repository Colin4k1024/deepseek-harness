//! Edge node — management and task offloading.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// Edge node configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeNodeConfig {
    pub name: String,
    pub endpoint: String,
    pub capabilities: Vec<String>,
    pub max_concurrent_tasks: u32,
}

/// An edge node in the network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeNode {
    pub id: String,
    pub config: EdgeNodeConfig,
    pub healthy: bool,
    pub active_tasks: u32,
    pub load_percent: u32,
}

/// Edge registry — manages edge nodes.
pub struct EdgeRegistry {
    nodes: RwLock<HashMap<String, EdgeNode>>,
}

impl EdgeRegistry {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
        }
    }

    /// Register an edge node.
    pub fn register(&self, node: EdgeNode) {
        let mut nodes = self.nodes.write().unwrap();
        nodes.insert(node.id.clone(), node);
    }

    /// Select the least-loaded healthy node with the required capability.
    pub fn select(&self, capability: &str) -> Option<EdgeNode> {
        let nodes = self.nodes.read().unwrap();
        nodes
            .values()
            .filter(|n| n.healthy && n.config.capabilities.iter().any(|c| c == capability))
            .min_by_key(|n| n.load_percent)
            .cloned()
    }

    /// Update node load.
    pub fn update_load(&self, id: &str, load: u32) {
        if let Some(n) = self.nodes.write().unwrap().get_mut(id) {
            n.load_percent = load;
        }
    }

    /// Mark node healthy/unhealthy.
    pub fn set_healthy(&self, id: &str, healthy: bool) {
        if let Some(n) = self.nodes.write().unwrap().get_mut(id) {
            n.healthy = healthy;
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.read().unwrap().len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.read().unwrap().is_empty()
    }
}

impl Default for EdgeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_node(id: &str, cap: &str, load: u32) -> EdgeNode {
        EdgeNode {
            id: id.into(),
            config: EdgeNodeConfig {
                name: id.into(),
                endpoint: "http://localhost".into(),
                capabilities: vec![cap.into()],
                max_concurrent_tasks: 10,
            },
            healthy: true,
            active_tasks: 0,
            load_percent: load,
        }
    }

    #[test]
    fn select_least_loaded() {
        let reg = EdgeRegistry::new();
        reg.register(test_node("n1", "gpu", 80));
        reg.register(test_node("n2", "gpu", 20));

        let selected = reg.select("gpu").unwrap();
        assert_eq!(selected.id, "n2");
    }

    #[test]
    fn capability_filter() {
        let reg = EdgeRegistry::new();
        reg.register(test_node("n1", "gpu", 50));
        reg.register(test_node("n2", "cpu", 10));

        assert!(reg.select("gpu").is_some());
        assert!(reg.select("tpu").is_none());
    }

    #[test]
    fn unhealthy_excluded() {
        let reg = EdgeRegistry::new();
        reg.register(test_node("n1", "gpu", 10));
        reg.set_healthy("n1", false);
        assert!(reg.select("gpu").is_none());
    }
}
