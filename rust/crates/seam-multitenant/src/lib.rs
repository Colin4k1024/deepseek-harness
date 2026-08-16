//! Multi-tenant isolation — tenant-scoped contexts and resource isolation.
//!
//! Each tenant gets an isolated Context with their own services,
//! session storage, and resource quotas. Cross-tenant access is
//! prevented at the framework level.

pub mod isolation;
pub mod tenant;

pub use isolation::{ResourceLimits, TenantContext};
pub use tenant::{Tenant, TenantConfig, TenantId, TenantManager, TenantState};
