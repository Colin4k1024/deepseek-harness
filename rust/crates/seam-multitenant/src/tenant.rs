//! Tenant management — create, manage, and isolate tenants.

use std::collections::HashMap;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

/// Unique tenant identifier.
pub type TenantId = String;

/// Tenant configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantConfig {
    /// Tenant name.
    pub name: String,

    /// Maximum number of concurrent sessions.
    pub max_sessions: u32,

    /// Maximum requests per day.
    pub daily_request_limit: u64,

    /// Maximum tokens per day.
    pub daily_token_limit: u64,

    /// Allowed LLM models (empty = all).
    pub allowed_models: Vec<String>,

    /// Custom metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Default for TenantConfig {
    fn default() -> Self {
        Self {
            name: "default".into(),
            max_sessions: 100,
            daily_request_limit: 10_000,
            daily_token_limit: 10_000_000,
            allowed_models: vec![],
            metadata: HashMap::new(),
        }
    }
}

/// Tenant state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TenantState {
    Active,
    Suspended,
    Disabled,
}

/// A tenant in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    /// Tenant ID.
    pub id: TenantId,

    /// Tenant configuration.
    pub config: TenantConfig,

    /// Current state.
    pub state: TenantState,

    /// Creation timestamp.
    pub created_at: u64,

    /// API keys associated with this tenant.
    pub api_keys: Vec<String>,
}

/// Tenant manager — manages tenant lifecycle and isolation.
pub struct TenantManager {
    tenants: DashMap<TenantId, Tenant>,
}

impl TenantManager {
    /// Create a new tenant manager.
    pub fn new() -> Self {
        Self {
            tenants: DashMap::new(),
        }
    }

    /// Create a new tenant.
    pub fn create(&self, id: impl Into<String>, config: TenantConfig) -> TenantId {
        let id = id.into();
        let tenant = Tenant {
            id: id.clone(),
            config,
            state: TenantState::Active,
            created_at: now(),
            api_keys: vec![],
        };
        self.tenants.insert(id.clone(), tenant);
        id
    }

    /// Get a tenant by ID.
    pub fn get(&self, tenant_id: &str) -> Option<Tenant> {
        self.tenants.get(tenant_id).map(|t| t.clone())
    }

    /// Update tenant configuration.
    pub fn update_config(&self, tenant_id: &str, config: TenantConfig) -> bool {
        if let Some(mut tenant) = self.tenants.get_mut(tenant_id) {
            tenant.config = config;
            true
        } else {
            false
        }
    }

    /// Suspend a tenant.
    pub fn suspend(&self, tenant_id: &str) -> bool {
        if let Some(mut tenant) = self.tenants.get_mut(tenant_id) {
            tenant.state = TenantState::Suspended;
            true
        } else {
            false
        }
    }

    /// Reactivate a tenant.
    pub fn activate(&self, tenant_id: &str) -> bool {
        if let Some(mut tenant) = self.tenants.get_mut(tenant_id) {
            tenant.state = TenantState::Active;
            true
        } else {
            false
        }
    }

    /// Disable a tenant (permanent).
    pub fn disable(&self, tenant_id: &str) -> bool {
        if let Some(mut tenant) = self.tenants.get_mut(tenant_id) {
            tenant.state = TenantState::Disabled;
            true
        } else {
            false
        }
    }

    /// Add an API key to a tenant.
    pub fn add_api_key(&self, tenant_id: &str, api_key: &str) -> bool {
        if let Some(mut tenant) = self.tenants.get_mut(tenant_id) {
            tenant.api_keys.push(api_key.into());
            true
        } else {
            false
        }
    }

    /// Find tenant by API key.
    pub fn find_by_api_key(&self, api_key: &str) -> Option<Tenant> {
        self.tenants.iter().find_map(|entry| {
            let tenant = entry.value();
            if tenant.api_keys.contains(&api_key.to_string()) && tenant.state == TenantState::Active
            {
                Some(tenant.clone())
            } else {
                None
            }
        })
    }

    /// List all tenants.
    pub fn list(&self) -> Vec<Tenant> {
        self.tenants.iter().map(|t| t.value().clone()).collect()
    }

    /// Number of tenants.
    pub fn len(&self) -> usize {
        self.tenants.len()
    }

    /// Whether the manager is empty.
    pub fn is_empty(&self) -> bool {
        self.tenants.is_empty()
    }
}

impl Default for TenantManager {
    fn default() -> Self {
        Self::new()
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_get() {
        let manager = TenantManager::new();
        let id = manager.create("tenant-1", TenantConfig::default());

        let tenant = manager.get(&id).unwrap();
        assert_eq!(tenant.id, "tenant-1");
        assert_eq!(tenant.state, TenantState::Active);
    }

    #[test]
    fn suspend_and_activate() {
        let manager = TenantManager::new();
        let id = manager.create("tenant-1", TenantConfig::default());

        manager.suspend(&id);
        assert_eq!(manager.get(&id).unwrap().state, TenantState::Suspended);

        manager.activate(&id);
        assert_eq!(manager.get(&id).unwrap().state, TenantState::Active);
    }

    #[test]
    fn find_by_api_key() {
        let manager = TenantManager::new();
        let id = manager.create("tenant-1", TenantConfig::default());
        manager.add_api_key(&id, "sk-tenant1-key");

        let tenant = manager.find_by_api_key("sk-tenant1-key").unwrap();
        assert_eq!(tenant.id, "tenant-1");

        assert!(manager.find_by_api_key("sk-unknown").is_none());
    }

    #[test]
    fn suspended_tenant_not_found_by_key() {
        let manager = TenantManager::new();
        let id = manager.create("tenant-1", TenantConfig::default());
        manager.add_api_key(&id, "sk-tenant1-key");
        manager.suspend(&id);

        assert!(manager.find_by_api_key("sk-tenant1-key").is_none());
    }

    #[test]
    fn list_tenants() {
        let manager = TenantManager::new();
        manager.create("a", TenantConfig::default());
        manager.create("b", TenantConfig::default());

        assert_eq!(manager.len(), 2);
        assert_eq!(manager.list().len(), 2);
    }
}
