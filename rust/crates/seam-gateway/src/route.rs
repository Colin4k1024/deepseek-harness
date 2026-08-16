//! Route — request routing and matching.

use serde::{Deserialize, Serialize};

/// HTTP method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Any,
}

/// Route definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    /// Path pattern (e.g., "/api/v1/chat", "/api/v1/*").
    pub path: String,

    /// HTTP method (Any = all methods).
    pub method: Method,

    /// Target backend URL.
    pub backend: String,

    /// Route name.
    pub name: String,

    /// Whether this route requires authentication.
    pub auth_required: bool,
}

impl Route {
    /// Create a new route.
    pub fn new(
        path: impl Into<String>,
        backend: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            method: Method::Any,
            backend: backend.into(),
            name: name.into(),
            auth_required: true,
        }
    }

    /// Set the HTTP method.
    pub fn with_method(mut self, method: Method) -> Self {
        self.method = method;
        self
    }

    /// Mark as public (no auth required).
    pub fn public(mut self) -> Self {
        self.auth_required = false;
        self
    }
}

/// Result of matching a request against routes.
#[derive(Debug, Clone)]
pub struct RouteMatch {
    /// Matched route.
    pub route: Route,

    /// Extracted path parameters.
    pub params: std::collections::HashMap<String, String>,
}

/// Match a request path against a route pattern.
pub fn match_route(routes: &[Route], path: &str, method: &Method) -> Option<RouteMatch> {
    for route in routes {
        if route.method != Method::Any && route.method != *method {
            continue;
        }

        if path_matches(&route.path, path) {
            return Some(RouteMatch {
                route: route.clone(),
                params: extract_params(&route.path, path),
            });
        }
    }
    None
}

/// Check if a path matches a pattern (supports `*` wildcards).
fn path_matches(pattern: &str, path: &str) -> bool {
    if pattern == path {
        return true;
    }

    if let Some(prefix) = pattern.strip_suffix("/*") {
        return path.starts_with(prefix);
    }

    false
}

/// Extract path parameters (placeholder for future :param support).
fn extract_params(_pattern: &str, _path: &str) -> std::collections::HashMap<String, String> {
    std::collections::HashMap::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match() {
        let routes = vec![Route::new("/api/v1/chat", "backend", "chat")];
        let result = match_route(&routes, "/api/v1/chat", &Method::Post);
        assert!(result.is_some());
        assert_eq!(result.unwrap().route.name, "chat");
    }

    #[test]
    fn wildcard_match() {
        let routes = vec![Route::new("/api/v1/*", "backend", "api")];
        assert!(match_route(&routes, "/api/v1/chat", &Method::Get).is_some());
        assert!(match_route(&routes, "/api/v1/health", &Method::Get).is_some());
        assert!(match_route(&routes, "/api/v2/chat", &Method::Get).is_none());
    }

    #[test]
    fn method_filtering() {
        let routes = vec![Route::new("/api/v1/chat", "backend", "chat").with_method(Method::Post)];
        assert!(match_route(&routes, "/api/v1/chat", &Method::Post).is_some());
        assert!(match_route(&routes, "/api/v1/chat", &Method::Get).is_none());
    }

    #[test]
    fn no_match() {
        let routes = vec![Route::new("/api/v1/chat", "backend", "chat")];
        assert!(match_route(&routes, "/other/path", &Method::Get).is_none());
    }
}
