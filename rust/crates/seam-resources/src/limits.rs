//! Resource limits configuration.

use serde::{Deserialize, Serialize};

/// Resource limit configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConfig {
    /// Maximum memory usage in bytes.
    pub max_memory_bytes: u64,

    /// Maximum CPU usage percentage (0-100).
    pub max_cpu_percent: u32,

    /// Maximum number of open file descriptors.
    pub max_file_descriptors: u32,

    /// Maximum number of concurrent tasks.
    pub max_concurrent_tasks: u32,

    /// Maximum number of open network connections.
    pub max_connections: u32,
}

impl Default for ResourceConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 512 * 1024 * 1024, // 512MB
            max_cpu_percent: 80,
            max_file_descriptors: 1024,
            max_concurrent_tasks: 100,
            max_connections: 1000,
        }
    }
}

/// Resource limits — enforces resource constraints.
pub struct ResourceLimits {
    config: ResourceConfig,
}

impl ResourceLimits {
    /// Create new resource limits.
    pub fn new(config: ResourceConfig) -> Self {
        Self { config }
    }

    /// Get the configuration.
    pub fn config(&self) -> &ResourceConfig {
        &self.config
    }

    /// Check if memory usage is within limits.
    pub fn check_memory(&self, current_bytes: u64) -> bool {
        current_bytes <= self.config.max_memory_bytes
    }

    /// Check if CPU usage is within limits.
    pub fn check_cpu(&self, current_percent: u32) -> bool {
        current_percent <= self.config.max_cpu_percent
    }

    /// Check if file descriptor count is within limits.
    pub fn check_file_descriptors(&self, current: u32) -> bool {
        current <= self.config.max_file_descriptors
    }

    /// Check if concurrent task count is within limits.
    pub fn check_concurrent_tasks(&self, current: u32) -> bool {
        current <= self.config.max_concurrent_tasks
    }

    /// Check if connection count is within limits.
    pub fn check_connections(&self, current: u32) -> bool {
        current <= self.config.max_connections
    }

    /// Get memory usage percentage.
    pub fn memory_percent(&self, current_bytes: u64) -> f64 {
        (current_bytes as f64 / self.config.max_memory_bytes as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limits() {
        let limits = ResourceLimits::new(ResourceConfig::default());
        assert!(limits.check_memory(100 * 1024 * 1024)); // 100MB
        assert!(!limits.check_memory(600 * 1024 * 1024)); // 600MB
    }

    #[test]
    fn cpu_check() {
        let limits = ResourceLimits::new(ResourceConfig::default());
        assert!(limits.check_cpu(50));
        assert!(!limits.check_cpu(90));
    }

    #[test]
    fn memory_percent() {
        let limits = ResourceLimits::new(ResourceConfig {
            max_memory_bytes: 1000,
            ..Default::default()
        });
        assert!((limits.memory_percent(500) - 50.0).abs() < 0.01);
    }
}
