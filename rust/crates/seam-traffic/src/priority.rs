//! Priority routing — route requests by priority level.

use serde::{Deserialize, Serialize};

/// Request priority.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// Critical requests (health checks, admin).
    Critical = 0,
    /// High priority (paid users, SLA-bound).
    High = 1,
    /// Normal priority (standard requests).
    #[default]
    Normal = 2,
    /// Low priority (background tasks, batch).
    Low = 3,
}

/// Priority routing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityConfig {
    /// Maximum concurrent requests per priority level.
    pub max_concurrent: std::collections::HashMap<Priority, u32>,

    /// Whether to shed low-priority requests under load.
    pub shed_low_priority: bool,

    /// Load threshold to start shedding (0-100).
    pub shed_threshold: u32,
}

impl Default for PriorityConfig {
    fn default() -> Self {
        let mut max_concurrent = std::collections::HashMap::new();
        max_concurrent.insert(Priority::Critical, 1000);
        max_concurrent.insert(Priority::High, 500);
        max_concurrent.insert(Priority::Normal, 200);
        max_concurrent.insert(Priority::Low, 50);

        Self {
            max_concurrent,
            shed_low_priority: true,
            shed_threshold: 80,
        }
    }
}

/// Priority router — decides whether to accept a request based on priority and load.
pub struct PriorityRouter {
    config: PriorityConfig,
    current_load: std::sync::atomic::AtomicU32,
    active_by_priority: std::sync::RwLock<std::collections::HashMap<Priority, u32>>,
}

impl PriorityRouter {
    /// Create a new priority router.
    pub fn new(config: PriorityConfig) -> Self {
        Self {
            config,
            current_load: std::sync::atomic::AtomicU32::new(0),
            active_by_priority: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Check if a request with the given priority should be accepted.
    pub fn should_accept(&self, priority: Priority) -> bool {
        let load = self.current_load.load(std::sync::atomic::Ordering::Relaxed);

        // Always accept critical requests
        if priority == Priority::Critical {
            return true;
        }

        // Shed low priority under high load
        if self.config.shed_low_priority
            && priority == Priority::Low
            && load >= self.config.shed_threshold
        {
            return false;
        }

        // Check per-priority limits
        let max = self
            .config
            .max_concurrent
            .get(&priority)
            .copied()
            .unwrap_or(100);
        let active = self.active_by_priority.read().unwrap();
        let current = active.get(&priority).copied().unwrap_or(0);
        current < max
    }

    /// Record a request starting.
    pub fn request_start(&self, priority: Priority) {
        self.current_load
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut active = self.active_by_priority.write().unwrap();
        *active.entry(priority).or_insert(0) += 1;
    }

    /// Record a request completing.
    pub fn request_end(&self, priority: Priority) {
        self.current_load
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        let mut active = self.active_by_priority.write().unwrap();
        if let Some(count) = active.get_mut(&priority) {
            *count = count.saturating_sub(1);
        }
    }

    /// Get current load.
    pub fn load(&self) -> u32 {
        self.current_load.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_normal_priority() {
        let router = PriorityRouter::new(PriorityConfig::default());
        assert!(router.should_accept(Priority::Normal));
    }

    #[test]
    fn always_accept_critical() {
        let config = PriorityConfig {
            shed_low_priority: true,
            shed_threshold: 0,
            ..Default::default()
        };
        let router = PriorityRouter::new(config);

        // Simulate high load
        for _ in 0..1000 {
            router.request_start(Priority::Normal);
        }

        assert!(router.should_accept(Priority::Critical));
    }

    #[test]
    fn shed_low_priority_under_load() {
        let config = PriorityConfig {
            shed_low_priority: true,
            shed_threshold: 10,
            ..Default::default()
        };
        let router = PriorityRouter::new(config);

        // Simulate load above threshold
        for _ in 0..15 {
            router.request_start(Priority::Normal);
        }

        assert!(!router.should_accept(Priority::Low));
        assert!(router.should_accept(Priority::Normal));
        assert!(router.should_accept(Priority::High));
    }

    #[test]
    fn per_priority_limits() {
        let config = PriorityConfig {
            max_concurrent: {
                let mut m = std::collections::HashMap::new();
                m.insert(Priority::Normal, 2);
                m
            },
            shed_low_priority: false,
            shed_threshold: 100,
        };
        let router = PriorityRouter::new(config);

        router.request_start(Priority::Normal);
        router.request_start(Priority::Normal);
        assert!(!router.should_accept(Priority::Normal));
    }
}
