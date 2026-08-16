//! Tenant isolation — tenant-scoped contexts and resource limits.

use std::sync::Arc;

use dsh_core_context::Context;

use crate::tenant::{Tenant, TenantConfig, TenantId};

/// Resource limits for a tenant.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum concurrent sessions.
    pub max_sessions: u32,

    /// Maximum requests per day.
    pub daily_request_limit: u64,

    /// Maximum tokens per day.
    pub daily_token_limit: u64,

    /// Maximum memory usage (bytes).
    pub max_memory_bytes: u64,
}

impl From<&TenantConfig> for ResourceLimits {
    fn from(config: &TenantConfig) -> Self {
        Self {
            max_sessions: config.max_sessions,
            daily_request_limit: config.daily_request_limit,
            daily_token_limit: config.daily_token_limit,
            max_memory_bytes: 100 * 1024 * 1024, // 100MB default
        }
    }
}

/// Tenant-scoped context — isolates tenant resources.
///
/// Each tenant gets their own Context with isolated services,
/// preventing cross-tenant access.
pub struct TenantContext {
    /// Tenant ID.
    pub tenant_id: TenantId,

    /// Tenant-scoped context.
    pub context: Context,

    /// Resource limits.
    pub limits: ResourceLimits,

    /// Current session count.
    session_count: std::sync::atomic::AtomicU32,
}

impl TenantContext {
    /// Create a new tenant context.
    pub fn new(tenant: &Tenant) -> Self {
        Self {
            tenant_id: tenant.id.clone(),
            context: Context::new(),
            limits: ResourceLimits::from(&tenant.config),
            session_count: std::sync::atomic::AtomicU32::new(0),
        }
    }

    /// Check if a new session can be created.
    pub fn can_create_session(&self) -> bool {
        let current = self
            .session_count
            .load(std::sync::atomic::Ordering::Relaxed);
        current < self.limits.max_sessions
    }

    /// Increment session count.
    pub fn add_session(&self) -> bool {
        let current = self
            .session_count
            .load(std::sync::atomic::Ordering::Relaxed);
        if current < self.limits.max_sessions {
            self.session_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Decrement session count.
    pub fn remove_session(&self) {
        self.session_count
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Get current session count.
    pub fn session_count(&self) -> u32 {
        self.session_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Tenant isolation manager — manages tenant contexts.
pub struct TenantIsolation {
    contexts: dashmap::DashMap<TenantId, Arc<TenantContext>>,
}

impl TenantIsolation {
    /// Create a new tenant isolation manager.
    pub fn new() -> Self {
        Self {
            contexts: dashmap::DashMap::new(),
        }
    }

    /// Create a context for a tenant.
    pub fn create_context(&self, tenant: &Tenant) -> Arc<TenantContext> {
        let ctx = Arc::new(TenantContext::new(tenant));
        self.contexts.insert(tenant.id.clone(), ctx.clone());
        ctx
    }

    /// Get a tenant's context.
    pub fn get_context(&self, tenant_id: &str) -> Option<Arc<TenantContext>> {
        self.contexts.get(tenant_id).map(|c| c.value().clone())
    }

    /// Remove a tenant's context.
    pub fn remove_context(&self, tenant_id: &str) -> bool {
        self.contexts.remove(tenant_id).is_some()
    }

    /// Number of tenant contexts.
    pub fn len(&self) -> usize {
        self.contexts.len()
    }

    /// Whether there are no tenant contexts.
    pub fn is_empty(&self) -> bool {
        self.contexts.is_empty()
    }
}

impl Default for TenantIsolation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tenant::TenantConfig;

    fn test_tenant(id: &str) -> Tenant {
        Tenant {
            id: id.into(),
            config: TenantConfig {
                max_sessions: 5,
                daily_request_limit: 100,
                daily_token_limit: 10000,
                ..Default::default()
            },
            state: crate::tenant::TenantState::Active,
            created_at: 0,
            api_keys: vec![],
        }
    }

    #[tokio::test]
    async fn tenant_context_session_limit() {
        let tenant = test_tenant("t1");
        let ctx = TenantContext::new(&tenant);

        assert!(ctx.can_create_session());
        for _ in 0..5 {
            assert!(ctx.add_session());
        }
        assert!(!ctx.can_create_session());
        assert!(!ctx.add_session());

        ctx.remove_session();
        assert!(ctx.can_create_session());
    }

    #[tokio::test]
    async fn isolation_manager() {
        let isolation = TenantIsolation::new();
        let tenant = test_tenant("t1");

        let ctx = isolation.create_context(&tenant);
        assert_eq!(ctx.tenant_id, "t1");

        let retrieved = isolation.get_context("t1").unwrap();
        assert_eq!(retrieved.tenant_id, "t1");

        assert!(isolation.remove_context("t1"));
        assert!(isolation.get_context("t1").is_none());
    }

    #[tokio::test]
    async fn cross_tenant_isolation() {
        let isolation = TenantIsolation::new();
        let t1 = test_tenant("t1");
        let t2 = test_tenant("t2");

        let ctx1 = isolation.create_context(&t1);
        let ctx2 = isolation.create_context(&t2);

        // Each tenant has their own context
        assert_eq!(ctx1.tenant_id, "t1");
        assert_eq!(ctx2.tenant_id, "t2");

        // Contexts are separate
        assert!(Arc::ptr_eq(&isolation.get_context("t1").unwrap(), &ctx1));
        assert!(Arc::ptr_eq(&isolation.get_context("t2").unwrap(), &ctx2));
    }
}
