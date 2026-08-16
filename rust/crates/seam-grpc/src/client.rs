//! gRPC client — typed RPC calls with metadata propagation.

use serde::{Deserialize, Serialize};

/// gRPC call result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResult<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub metadata: std::collections::HashMap<String, String>,
}

/// gRPC client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcClientConfig {
    pub endpoint: String,
    pub timeout_ms: u64,
    pub max_retries: u32,
}

impl Default for GrpcClientConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:50051".into(),
            timeout_ms: 5000,
            max_retries: 3,
        }
    }
}

/// gRPC client.
pub struct GrpcClient {
    config: GrpcClientConfig,
}

impl GrpcClient {
    pub fn new(config: GrpcClientConfig) -> Self {
        Self { config }
    }

    pub fn endpoint(&self) -> &str {
        &self.config.endpoint
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_config() {
        let config = GrpcClientConfig::default();
        assert_eq!(config.endpoint, "http://localhost:50051");
        assert_eq!(config.timeout_ms, 5000);
    }

    #[test]
    fn rpc_result() {
        let result: RpcResult<String> = RpcResult {
            success: true,
            data: Some("ok".into()),
            error: None,
            metadata: std::collections::HashMap::new(),
        };
        assert!(result.success);
    }
}
