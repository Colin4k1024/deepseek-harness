//! gRPC service definition.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// gRPC method definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodDef {
    pub name: String,
    pub input_type: String,
    pub output_type: String,
    pub streaming: bool,
}

/// Trait for gRPC services.
#[async_trait]
pub trait GrpcService: Send + Sync {
    /// Service name.
    fn name(&self) -> &str;

    /// Available methods.
    fn methods(&self) -> Vec<MethodDef>;
}

/// Metadata for gRPC calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcMetadata {
    pub headers: std::collections::HashMap<String, String>,
}

impl GrpcMetadata {
    pub fn new() -> Self {
        Self {
            headers: std::collections::HashMap::new(),
        }
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
}

impl Default for GrpcMetadata {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_headers() {
        let meta = GrpcMetadata::new().with_header("x-trace-id", "abc123");
        assert_eq!(meta.headers.get("x-trace-id").unwrap(), "abc123");
    }
}
