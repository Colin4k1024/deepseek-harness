//! Scaling policy — defines when to scale up or down.

use serde::{Deserialize, Serialize};

/// Scale direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScaleDirection {
    /// Scale up (add instances).
    Up,
    /// Scale down (remove instances).
    Down,
    /// No scaling needed.
    None,
}

/// Scaling policy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalePolicy {
    /// Minimum number of instances.
    pub min_instances: u32,

    /// Maximum number of instances.
    pub max_instances: u32,

    /// CPU threshold to scale up (0-100).
    pub scale_up_cpu: u32,

    /// CPU threshold to scale down (0-100).
    pub scale_down_cpu: u32,

    /// Memory threshold to scale up (0-100).
    pub scale_up_memory: u32,

    /// Requests per second per instance threshold to scale up.
    pub scale_up_rps: f64,

    /// Cooldown period in seconds between scaling events.
    pub cooldown_secs: u64,
}

impl Default for ScalePolicy {
    fn default() -> Self {
        Self {
            min_instances: 1,
            max_instances: 10,
            scale_up_cpu: 70,
            scale_down_cpu: 30,
            scale_up_memory: 80,
            scale_up_rps: 100.0,
            cooldown_secs: 60,
        }
    }
}

impl ScalePolicy {
    /// Evaluate whether scaling is needed.
    pub fn evaluate(
        &self,
        current_instances: u32,
        cpu_percent: u32,
        memory_percent: u32,
        rps: f64,
    ) -> ScaleDirection {
        // Scale up conditions
        if current_instances < self.max_instances
            && (cpu_percent >= self.scale_up_cpu
                || memory_percent >= self.scale_up_memory
                || rps / current_instances as f64 >= self.scale_up_rps)
        {
            return ScaleDirection::Up;
        }

        // Scale down conditions
        if current_instances > self.min_instances
            && cpu_percent <= self.scale_down_cpu
            && memory_percent < self.scale_up_memory
        {
            return ScaleDirection::Down;
        }

        ScaleDirection::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_up_on_high_cpu() {
        let policy = ScalePolicy::default();
        assert_eq!(policy.evaluate(2, 80, 50, 10.0), ScaleDirection::Up);
    }

    #[test]
    fn scale_down_on_low_cpu() {
        let policy = ScalePolicy::default();
        assert_eq!(policy.evaluate(3, 20, 30, 10.0), ScaleDirection::Down);
    }

    #[test]
    fn no_scale_in_range() {
        let policy = ScalePolicy::default();
        assert_eq!(policy.evaluate(2, 50, 50, 10.0), ScaleDirection::None);
    }

    #[test]
    fn respect_max_instances() {
        let policy = ScalePolicy {
            max_instances: 2,
            ..Default::default()
        };
        assert_eq!(policy.evaluate(2, 90, 90, 1000.0), ScaleDirection::None);
    }

    #[test]
    fn respect_min_instances() {
        let policy = ScalePolicy {
            min_instances: 2,
            ..Default::default()
        };
        assert_eq!(policy.evaluate(2, 10, 10, 1.0), ScaleDirection::None);
    }
}
