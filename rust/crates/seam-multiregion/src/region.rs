//! Multi-region — region-aware routing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Region configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionConfig {
    pub name: String,
    pub endpoint: String,
    pub primary: bool,
    pub healthy: bool,
}

/// A deployment region.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub id: String,
    pub config: RegionConfig,
    pub latency_ms: u32,
    pub load_percent: u32,
}

/// Region router — routes requests to optimal region.
pub struct RegionRouter {
    regions: HashMap<String, Region>,
}

impl RegionRouter {
    pub fn new() -> Self {
        Self {
            regions: HashMap::new(),
        }
    }

    /// Register a region.
    pub fn register(&mut self, region: Region) {
        self.regions.insert(region.id.clone(), region);
    }

    /// Select the best region (lowest latency + healthy).
    pub fn select(&self) -> Option<&Region> {
        self.regions
            .values()
            .filter(|r| r.config.healthy)
            .min_by_key(|r| r.latency_ms)
    }

    /// Select a specific region by ID.
    pub fn get(&self, id: &str) -> Option<&Region> {
        self.regions.get(id)
    }

    /// List healthy regions.
    pub fn healthy(&self) -> Vec<&Region> {
        self.regions.values().filter(|r| r.config.healthy).collect()
    }

    /// Failover: select next best region if primary is down.
    pub fn failover(&self, failed_id: &str) -> Option<&Region> {
        self.regions
            .values()
            .filter(|r| r.config.healthy && r.id != failed_id)
            .min_by_key(|r| r.latency_ms)
    }
}

impl Default for RegionRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_region(id: &str, latency: u32, primary: bool) -> Region {
        Region {
            id: id.into(),
            config: RegionConfig {
                name: id.into(),
                endpoint: format!("https://{id}.example.com"),
                primary,
                healthy: true,
            },
            latency_ms: latency,
            load_percent: 0,
        }
    }

    #[test]
    fn select_best_region() {
        let mut router = RegionRouter::new();
        router.register(test_region("us-east", 10, true));
        router.register(test_region("eu-west", 50, false));

        let best = router.select().unwrap();
        assert_eq!(best.id, "us-east");
    }

    #[test]
    fn failover() {
        let mut router = RegionRouter::new();
        router.register(test_region("us-east", 10, true));
        router.register(test_region("eu-west", 50, false));

        let fallback = router.failover("us-east").unwrap();
        assert_eq!(fallback.id, "eu-west");
    }
}
