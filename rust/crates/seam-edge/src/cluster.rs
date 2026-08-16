//! Cluster — multi-cluster registry and routing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// Cluster configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub name: String,
    pub endpoint: String,
    pub region: String,
    pub weight: u32,
}

/// A registered cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    pub id: String,
    pub config: ClusterConfig,
    pub healthy: bool,
    pub latency_ms: u32,
}

/// Cluster registry — manages multiple clusters.
pub struct ClusterRegistry {
    clusters: RwLock<HashMap<String, Cluster>>,
}

impl ClusterRegistry {
    pub fn new() -> Self {
        Self {
            clusters: RwLock::new(HashMap::new()),
        }
    }

    /// Register a cluster.
    pub fn register(&self, cluster: Cluster) {
        let mut clusters = self.clusters.write().unwrap();
        clusters.insert(cluster.id.clone(), cluster);
    }

    /// Get a cluster by ID.
    pub fn get(&self, id: &str) -> Option<Cluster> {
        self.clusters.read().unwrap().get(id).cloned()
    }

    /// List healthy clusters.
    pub fn healthy_clusters(&self) -> Vec<Cluster> {
        self.clusters
            .read()
            .unwrap()
            .values()
            .filter(|c| c.healthy)
            .cloned()
            .collect()
    }

    /// Select cluster by region preference.
    pub fn select_by_region(&self, region: &str) -> Option<Cluster> {
        let clusters = self.clusters.read().unwrap();
        clusters
            .values()
            .filter(|c| c.healthy && c.config.region == region)
            .min_by_key(|c| c.latency_ms)
            .cloned()
    }

    /// Update cluster health.
    pub fn set_healthy(&self, id: &str, healthy: bool) {
        if let Some(c) = self.clusters.write().unwrap().get_mut(id) {
            c.healthy = healthy;
        }
    }

    pub fn len(&self) -> usize {
        self.clusters.read().unwrap().len()
    }
    pub fn is_empty(&self) -> bool {
        self.clusters.read().unwrap().is_empty()
    }
}

impl Default for ClusterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cluster(id: &str, region: &str) -> Cluster {
        Cluster {
            id: id.into(),
            config: ClusterConfig {
                name: id.into(),
                endpoint: "http://localhost".into(),
                region: region.into(),
                weight: 1,
            },
            healthy: true,
            latency_ms: 10,
        }
    }

    #[test]
    fn register_and_select() {
        let reg = ClusterRegistry::new();
        reg.register(test_cluster("us-east", "us-east-1"));
        reg.register(test_cluster("eu-west", "eu-west-1"));

        assert_eq!(reg.len(), 2);
        assert!(reg.select_by_region("us-east-1").is_some());
    }

    #[test]
    fn health_filter() {
        let reg = ClusterRegistry::new();
        reg.register(test_cluster("c1", "us-east-1"));
        reg.set_healthy("c1", false);
        assert!(reg.healthy_clusters().is_empty());
    }
}
