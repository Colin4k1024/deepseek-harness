//! Mobile client configuration for SDK integration.

use serde::{Deserialize, Serialize};

/// Mobile client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileClientConfig {
    /// Server base URL.
    pub base_url: String,

    /// API key.
    pub api_key: String,

    /// Client name (e.g., "dsh-ios", "dsh-android").
    pub client_name: String,

    /// Client version.
    pub client_version: String,

    /// Whether to use compression.
    pub use_compression: bool,

    /// Connection timeout in seconds.
    pub timeout_secs: u32,

    /// Retry count.
    pub retry_count: u32,
}

impl Default for MobileClientConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:3000".into(),
            api_key: String::new(),
            client_name: "dsh-mobile".into(),
            client_version: "0.1.0".into(),
            use_compression: true,
            timeout_secs: 30,
            retry_count: 3,
        }
    }
}

/// Mobile client for SDK integration.
pub struct MobileClient {
    config: MobileClientConfig,
}

impl MobileClient {
    /// Create a new mobile client.
    pub fn new(config: MobileClientConfig) -> Self {
        Self { config }
    }

    /// Get the API URL for a given endpoint.
    pub fn api_url(&self, endpoint: &str) -> String {
        format!("{}{}", self.config.base_url, endpoint)
    }

    /// Get the health check URL.
    pub fn health_url(&self) -> String {
        self.api_url("/health")
    }

    /// Get the chat URL.
    pub fn chat_url(&self) -> String {
        self.api_url("/api/v1/chat")
    }

    /// Get the streaming chat URL.
    pub fn chat_stream_url(&self) -> String {
        self.api_url("/api/v1/chat/stream")
    }

    /// Get request headers.
    pub fn headers(&self) -> Vec<(String, String)> {
        vec![
            (
                "Authorization".into(),
                format!("Bearer {}", self.config.api_key),
            ),
            ("Content-Type".into(), "application/json".into()),
            ("X-Client-Name".into(), self.config.client_name.clone()),
            (
                "X-Client-Version".into(),
                self.config.client_version.clone(),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_url() {
        let config = MobileClientConfig {
            base_url: "https://api.example.com".into(),
            ..Default::default()
        };
        let client = MobileClient::new(config);
        assert_eq!(client.chat_url(), "https://api.example.com/api/v1/chat");
    }

    #[test]
    fn headers() {
        let config = MobileClientConfig {
            api_key: "sk-test".into(),
            client_name: "dsh-ios".into(),
            ..Default::default()
        };
        let client = MobileClient::new(config);
        let headers = client.headers();
        assert!(
            headers
                .iter()
                .any(|(k, v)| k == "Authorization" && v == "Bearer sk-test")
        );
        assert!(
            headers
                .iter()
                .any(|(k, v)| k == "X-Client-Name" && v == "dsh-ios")
        );
    }
}
