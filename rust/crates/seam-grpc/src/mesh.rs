//! Service mesh — service discovery and routing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// Service entry in the mesh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    pub id: String,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub healthy: bool,
    pub metadata: HashMap<String, String>,
}

/// Mesh configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshConfig {
    pub health_check_interval_secs: u64,
    pub max_retries: u32,
    pub timeout_ms: u64,
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            health_check_interval_secs: 10,
            max_retries: 3,
            timeout_ms: 5000,
        }
    }
}

/// Service mesh — manages service discovery and routing.
pub struct ServiceMesh {
    services: RwLock<HashMap<String, Vec<ServiceEntry>>>,
}

impl ServiceMesh {
    pub fn new() -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
        }
    }

    /// Register a service.
    pub fn register(&self, entry: ServiceEntry) {
        let mut services = self.services.write().unwrap();
        services.entry(entry.name.clone()).or_default().push(entry);
    }

    /// Deregister a service.
    pub fn deregister(&self, service_name: &str, service_id: &str) {
        let mut services = self.services.write().unwrap();
        if let Some(entries) = services.get_mut(service_name) {
            entries.retain(|e| e.id != service_id);
        }
    }

    /// Get healthy instances of a service.
    pub fn resolve(&self, service_name: &str) -> Vec<ServiceEntry> {
        let services = self.services.read().unwrap();
        services
            .get(service_name)
            .map(|entries| entries.iter().filter(|e| e.healthy).cloned().collect())
            .unwrap_or_default()
    }

    /// Simple round-robin selection.
    pub fn select(&self, service_name: &str) -> Option<ServiceEntry> {
        let instances = self.resolve(service_name);
        if instances.is_empty() {
            return None;
        }
        let idx = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as usize
            % instances.len();
        Some(instances[idx].clone())
    }

    /// List all registered services.
    pub fn list_services(&self) -> Vec<String> {
        self.services.read().unwrap().keys().cloned().collect()
    }
}

impl Default for ServiceMesh {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_entry(id: &str, name: &str) -> ServiceEntry {
        ServiceEntry {
            id: id.into(),
            name: name.into(),
            address: "127.0.0.1".into(),
            port: 8080,
            healthy: true,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn register_and_resolve() {
        let mesh = ServiceMesh::new();
        mesh.register(test_entry("a1", "api"));
        mesh.register(test_entry("a2", "api"));
        assert_eq!(mesh.resolve("api").len(), 2);
    }

    #[test]
    fn deregister() {
        let mesh = ServiceMesh::new();
        mesh.register(test_entry("a1", "api"));
        mesh.deregister("api", "a1");
        assert!(mesh.resolve("api").is_empty());
    }

    #[test]
    fn unhealthy_filtered() {
        let mesh = ServiceMesh::new();
        let mut entry = test_entry("a1", "api");
        entry.healthy = false;
        mesh.register(entry);
        assert!(mesh.resolve("api").is_empty());
    }

    #[test]
    fn select_instance() {
        let mesh = ServiceMesh::new();
        mesh.register(test_entry("a1", "api"));
        assert!(mesh.select("api").is_some());
        assert!(mesh.select("nonexistent").is_none());
    }
}
