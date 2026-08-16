//! L2: Auth & RBAC seam — authentication, rate limiting, and quota management.
//!
//! Provides:
//! - API key authentication (hashed storage)
//! - Rate limiting (token bucket algorithm)
//! - Quota management (daily/monthly limits)

pub mod auth;
pub mod quota;
pub mod rate_limit;

pub use auth::{AuthError, AuthManager, Credentials};
pub use quota::{QuotaConfig, QuotaManager};
pub use rate_limit::{RateLimitConfig, RateLimiter};
