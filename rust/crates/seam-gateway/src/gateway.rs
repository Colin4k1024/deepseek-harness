//! Gateway — request routing and proxying.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::middleware::{Middleware, MiddlewareResult, RequestContext};
use super::route::{Method, Route, RouteMatch, match_route};

/// Gateway errors.
#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("no route found for {method} {path}")]
    NoRoute { method: String, path: String },
    #[error("backend error: {0}")]
    Backend(String),
    #[error("middleware rejected: {0}")]
    MiddlewareRejected(String),
    #[error("configuration error: {0}")]
    Config(String),
}

/// Gateway configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Listen address.
    pub listen: String,

    /// Routes.
    pub routes: Vec<Route>,

    /// Rate limit (requests per second).
    pub rate_limit: u32,

    /// Request timeout in seconds.
    pub timeout_secs: u64,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            listen: "0.0.0.0:3000".into(),
            routes: Vec::new(),
            rate_limit: 100,
            timeout_secs: 30,
        }
    }
}

/// API Gateway.
pub struct Gateway {
    config: GatewayConfig,
    middleware: Vec<Box<dyn Middleware>>,
}

impl Gateway {
    /// Create a new gateway.
    pub fn new(config: GatewayConfig) -> Self {
        Self {
            config,
            middleware: Vec::new(),
        }
    }

    /// Add a middleware.
    pub fn add_middleware(&mut self, mw: Box<dyn Middleware>) {
        self.middleware.push(mw);
    }

    /// Process a request through the gateway.
    pub fn process(&self, ctx: &mut RequestContext) -> Result<RouteMatch, GatewayError> {
        // Run middleware chain
        for mw in &self.middleware {
            match mw.process(ctx) {
                MiddlewareResult::Continue => {}
                MiddlewareResult::ShortCircuit { status, body } => {
                    tracing::info!(middleware = mw.name(), status, "middleware short-circuit");
                    return Err(GatewayError::MiddlewareRejected(body));
                }
                MiddlewareResult::Reject { status, reason } => {
                    tracing::warn!(
                        middleware = mw.name(),
                        status,
                        reason,
                        "middleware rejected"
                    );
                    return Err(GatewayError::MiddlewareRejected(reason));
                }
            }
        }

        // Route the request
        let method = match ctx.method.as_str() {
            "GET" => Method::Get,
            "POST" => Method::Post,
            "PUT" => Method::Put,
            "DELETE" => Method::Delete,
            "PATCH" => Method::Patch,
            _ => Method::Any,
        };

        match_route(&self.config.routes, &ctx.path, &method).ok_or_else(|| GatewayError::NoRoute {
            method: ctx.method.clone(),
            path: ctx.path.clone(),
        })
    }

    /// Get the configuration.
    pub fn config(&self) -> &GatewayConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::super::middleware::RequestIdMiddleware;
    use super::*;

    #[test]
    fn route_matching() {
        let config = GatewayConfig {
            routes: vec![Route::new("/api/v1/chat", "http://backend:8080", "chat")],
            ..Default::default()
        };
        let gateway = Gateway::new(config);

        let mut ctx = RequestContext::new("/api/v1/chat", "POST");
        let result = gateway.process(&mut ctx);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().route.name, "chat");
    }

    #[test]
    fn no_route() {
        let config = GatewayConfig::default();
        let gateway = Gateway::new(config);

        let mut ctx = RequestContext::new("/unknown", "GET");
        let result = gateway.process(&mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn middleware_modifies_context() {
        let config = GatewayConfig {
            routes: vec![Route::new("/api/*", "http://backend", "api").public()],
            ..Default::default()
        };
        let mut gateway = Gateway::new(config);
        gateway.add_middleware(Box::new(RequestIdMiddleware));

        let mut ctx = RequestContext::new("/api/test", "GET");
        let _ = gateway.process(&mut ctx);
        assert!(ctx.headers.contains_key("x-request-id"));
    }
}
