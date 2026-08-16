//! Middleware — request/response processing chain.

use serde::{Deserialize, Serialize};

/// Middleware result.
#[derive(Debug, Clone)]
pub enum MiddlewareResult {
    /// Continue to next middleware.
    Continue,
    /// Short-circuit with a response.
    ShortCircuit { status: u16, body: String },
    /// Reject with an error.
    Reject { status: u16, reason: String },
}

/// Request context passed through middleware.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestContext {
    /// Request ID.
    pub request_id: String,

    /// Request path.
    pub path: String,

    /// HTTP method.
    pub method: String,

    /// Headers.
    pub headers: std::collections::HashMap<String, String>,

    /// Client IP.
    pub client_ip: String,

    /// Authenticated user ID (if any).
    pub user_id: Option<String>,

    /// Request timestamp.
    pub timestamp: u64,
}

impl RequestContext {
    /// Create a new request context.
    pub fn new(path: impl Into<String>, method: impl Into<String>) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            path: path.into(),
            method: method.into(),
            headers: std::collections::HashMap::new(),
            client_ip: "127.0.0.1".into(),
            user_id: None,
            timestamp: now(),
        }
    }
}

/// Trait for middleware.
pub trait Middleware: Send + Sync {
    /// Process a request. Return Continue to pass to next middleware.
    fn process(&self, ctx: &mut RequestContext) -> MiddlewareResult;

    /// Middleware name.
    fn name(&self) -> &str;
}

/// CORS middleware.
pub struct CorsMiddleware {
    pub allowed_origins: Vec<String>,
}

impl CorsMiddleware {
    pub fn new(allowed_origins: Vec<String>) -> Self {
        Self { allowed_origins }
    }
}

impl Middleware for CorsMiddleware {
    fn process(&self, ctx: &mut RequestContext) -> MiddlewareResult {
        ctx.headers.insert(
            "access-control-allow-origin".into(),
            self.allowed_origins
                .first()
                .cloned()
                .unwrap_or_else(|| "*".into()),
        );
        ctx.headers.insert(
            "access-control-allow-methods".into(),
            "GET, POST, PUT, DELETE".into(),
        );
        MiddlewareResult::Continue
    }

    fn name(&self) -> &str {
        "cors"
    }
}

/// Request ID middleware.
pub struct RequestIdMiddleware;

impl Middleware for RequestIdMiddleware {
    fn process(&self, ctx: &mut RequestContext) -> MiddlewareResult {
        ctx.headers
            .insert("x-request-id".into(), ctx.request_id.clone());
        MiddlewareResult::Continue
    }

    fn name(&self) -> &str {
        "request-id"
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
    fn cors_adds_headers() {
        let middleware = CorsMiddleware::new(vec!["https://example.com".into()]);
        let mut ctx = RequestContext::new("/api/test", "GET");
        let result = middleware.process(&mut ctx);

        assert!(matches!(result, MiddlewareResult::Continue));
        assert_eq!(
            ctx.headers.get("access-control-allow-origin").unwrap(),
            "https://example.com"
        );
    }

    #[test]
    fn request_id_adds_header() {
        let middleware = RequestIdMiddleware;
        let mut ctx = RequestContext::new("/api/test", "GET");
        let result = middleware.process(&mut ctx);

        assert!(matches!(result, MiddlewareResult::Continue));
        assert!(ctx.headers.contains_key("x-request-id"));
    }
}
