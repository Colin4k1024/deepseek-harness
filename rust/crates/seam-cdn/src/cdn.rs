//! CDN — content distribution and caching.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cache policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachePolicy {
    pub max_age_secs: u64,
    pub stale_while_revalidate_secs: u64,
    pub cache_control: String,
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            max_age_secs: 3600,
            stale_while_revalidate_secs: 300,
            cache_control: "public".into(),
        }
    }
}

/// CDN configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdnConfig {
    pub origin: String,
    pub regions: Vec<String>,
    pub default_policy: CachePolicy,
}

/// CDN router — routes requests to nearest edge.
pub struct CdnRouter {
    config: CdnConfig,
    #[allow(dead_code)] // Will be used for edge node caching
    region_cache: HashMap<String, String>,
}

impl CdnRouter {
    pub fn new(config: CdnConfig) -> Self {
        Self {
            config,
            region_cache: HashMap::new(),
        }
    }

    /// Get edge URL for a region.
    pub fn edge_url(&self, region: &str, path: &str) -> String {
        if self.config.regions.contains(&region.to_string()) {
            format!("https://{region}.cdn.example.com{path}")
        } else {
            format!("{}{path}", self.config.origin)
        }
    }

    /// Get cache headers for a policy.
    pub fn cache_headers(&self, policy: &CachePolicy) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert(
            "cache-control".into(),
            format!("{}, max-age={}", policy.cache_control, policy.max_age_secs),
        );
        headers.insert("x-cdn-region".into(), "auto".into());
        headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_url_routing() {
        let router = CdnRouter::new(CdnConfig {
            origin: "https://origin.example.com".into(),
            regions: vec!["us-east".into(), "eu-west".into()],
            default_policy: CachePolicy::default(),
        });

        assert!(router.edge_url("us-east", "/api/v1").contains("us-east"));
        assert!(router.edge_url("ap-south", "/api/v1").contains("origin"));
    }

    #[test]
    fn cache_headers() {
        let router = CdnRouter::new(CdnConfig {
            origin: "https://origin.example.com".into(),
            regions: vec![],
            default_policy: CachePolicy::default(),
        });

        let headers = router.cache_headers(&CachePolicy::default());
        assert!(headers.get("cache-control").unwrap().contains("max-age"));
    }
}
