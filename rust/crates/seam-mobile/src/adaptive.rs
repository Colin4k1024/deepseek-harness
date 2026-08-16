//! Adaptive content delivery for mobile devices.

use serde::{Deserialize, Serialize};

/// Device type detection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Desktop,
    Tablet,
    Mobile,
    Watch,
    #[default]
    Unknown,
}

/// Network condition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkCondition {
    /// High-speed connection (WiFi, 5G).
    #[default]
    Fast,
    /// Moderate connection (4G).
    Moderate,
    /// Slow connection (3G, congested).
    Slow,
    /// Offline.
    Offline,
}

/// Adaptive content configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveConfig {
    /// Device type.
    pub device: DeviceType,

    /// Network condition.
    pub network: NetworkCondition,

    /// Screen width in pixels.
    pub screen_width: u32,

    /// Screen height in pixels.
    pub screen_height: u32,

    /// Whether the device supports streaming.
    pub supports_streaming: bool,

    /// Maximum response size in bytes.
    pub max_response_bytes: usize,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            device: DeviceType::Desktop,
            network: NetworkCondition::Fast,
            screen_width: 1920,
            screen_height: 1080,
            supports_streaming: true,
            max_response_bytes: 1024 * 1024, // 1MB
        }
    }
}

/// Content adapter — adapts responses for the device.
pub struct ContentAdapter {
    config: AdaptiveConfig,
}

impl ContentAdapter {
    /// Create a new content adapter.
    pub fn new(config: AdaptiveConfig) -> Self {
        Self { config }
    }

    /// Adapt content for the current device.
    pub fn adapt(&self, content: &str) -> String {
        // For mobile devices, truncate very long responses
        if self.config.device == DeviceType::Mobile && content.len() > 10000 {
            let truncated: String = content.chars().take(10000).collect();
            return format!("{truncated}... (truncated for mobile)");
        }

        // For slow networks, simplify content
        if self.config.network == NetworkCondition::Slow && content.len() > 5000 {
            let truncated: String = content.chars().take(5000).collect();
            return format!("{truncated}... (simplified for slow connection)");
        }

        content.to_string()
    }

    /// Whether to use streaming for this device.
    pub fn should_stream(&self) -> bool {
        self.config.supports_streaming && self.config.network != NetworkCondition::Offline
    }

    /// Get the recommended chunk size for streaming.
    pub fn chunk_size(&self) -> usize {
        match self.config.network {
            NetworkCondition::Fast => 4096,
            NetworkCondition::Moderate => 2048,
            NetworkCondition::Slow => 512,
            NetworkCondition::Offline => 0,
        }
    }

    /// Whether the device is mobile.
    pub fn is_mobile(&self) -> bool {
        matches!(self.config.device, DeviceType::Mobile | DeviceType::Watch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_no_truncation() {
        let config = AdaptiveConfig::default();
        let adapter = ContentAdapter::new(config);
        let content = "a".repeat(20000);
        assert_eq!(adapter.adapt(&content), content);
    }

    #[test]
    fn mobile_truncates_long_content() {
        let config = AdaptiveConfig {
            device: DeviceType::Mobile,
            ..Default::default()
        };
        let adapter = ContentAdapter::new(config);
        let content = "a".repeat(15000);
        let adapted = adapter.adapt(&content);
        assert!(adapted.len() < content.len());
        assert!(adapted.contains("truncated for mobile"));
    }

    #[test]
    fn slow_network_simplifies() {
        let config = AdaptiveConfig {
            network: NetworkCondition::Slow,
            ..Default::default()
        };
        let adapter = ContentAdapter::new(config);
        let content = "a".repeat(8000);
        let adapted = adapter.adapt(&content);
        assert!(adapted.contains("simplified for slow connection"));
    }

    #[test]
    fn streaming_detection() {
        let adapter = ContentAdapter::new(AdaptiveConfig::default());
        assert!(adapter.should_stream());

        let adapter = ContentAdapter::new(AdaptiveConfig {
            supports_streaming: false,
            ..Default::default()
        });
        assert!(!adapter.should_stream());
    }

    #[test]
    fn chunk_size_by_network() {
        let adapter = ContentAdapter::new(AdaptiveConfig {
            network: NetworkCondition::Fast,
            ..Default::default()
        });
        assert_eq!(adapter.chunk_size(), 4096);

        let adapter = ContentAdapter::new(AdaptiveConfig {
            network: NetworkCondition::Slow,
            ..Default::default()
        });
        assert_eq!(adapter.chunk_size(), 512);
    }

    #[test]
    fn mobile_detection() {
        let adapter = ContentAdapter::new(AdaptiveConfig {
            device: DeviceType::Mobile,
            ..Default::default()
        });
        assert!(adapter.is_mobile());

        let adapter = ContentAdapter::new(AdaptiveConfig::default());
        assert!(!adapter.is_mobile());
    }
}
