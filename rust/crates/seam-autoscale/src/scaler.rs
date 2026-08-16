//! Auto-scaler — manages scaling decisions and events.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use super::policy::{ScaleDirection, ScalePolicy};

/// Scaling event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleEvent {
    /// Direction of scaling.
    pub direction: ScaleDirection,

    /// Instance count before scaling.
    pub before: u32,

    /// Instance count after scaling.
    pub after: u32,

    /// Reason for scaling.
    pub reason: String,

    /// Timestamp.
    pub timestamp: u64,
}

/// Auto-scaler — manages instance count based on metrics.
pub struct AutoScaler {
    policy: ScalePolicy,
    current_instances: AtomicU32,
    last_scale: RwLock<Instant>,
    events: RwLock<Vec<ScaleEvent>>,
}

impl AutoScaler {
    /// Create a new auto-scaler.
    pub fn new(policy: ScalePolicy) -> Self {
        let initial = policy.min_instances;
        Self {
            policy,
            current_instances: AtomicU32::new(initial),
            last_scale: RwLock::new(Instant::now()),
            events: RwLock::new(Vec::new()),
        }
    }

    /// Get current instance count.
    pub fn instances(&self) -> u32 {
        self.current_instances.load(Ordering::Relaxed)
    }

    /// Evaluate metrics and potentially scale.
    pub async fn evaluate(&self, cpu: u32, memory: u32, rps: f64) -> Option<ScaleEvent> {
        // Check cooldown
        let last_scale = self.last_scale.read().await;
        if last_scale.elapsed() < Duration::from_secs(self.policy.cooldown_secs) {
            return None;
        }
        drop(last_scale);

        let current = self.current_instances.load(Ordering::Relaxed);
        let direction = self.policy.evaluate(current, cpu, memory, rps);

        match direction {
            ScaleDirection::Up => {
                let new_count = (current + 1).min(self.policy.max_instances);
                self.current_instances.store(new_count, Ordering::Relaxed);
                *self.last_scale.write().await = Instant::now();

                let event = ScaleEvent {
                    direction: ScaleDirection::Up,
                    before: current,
                    after: new_count,
                    reason: format!("cpu={cpu}%, memory={memory}%, rps={rps}"),
                    timestamp: now(),
                };
                self.events.write().await.push(event.clone());
                Some(event)
            }
            ScaleDirection::Down => {
                let new_count = (current - 1).max(self.policy.min_instances);
                self.current_instances.store(new_count, Ordering::Relaxed);
                *self.last_scale.write().await = Instant::now();

                let event = ScaleEvent {
                    direction: ScaleDirection::Down,
                    before: current,
                    after: new_count,
                    reason: format!("cpu={cpu}%, memory={memory}%"),
                    timestamp: now(),
                };
                self.events.write().await.push(event.clone());
                Some(event)
            }
            ScaleDirection::None => None,
        }
    }

    /// Get scaling events.
    pub async fn events(&self) -> Vec<ScaleEvent> {
        self.events.read().await.clone()
    }

    /// Manually set instance count.
    pub fn set_instances(&self, count: u32) {
        let clamped = count
            .max(self.policy.min_instances)
            .min(self.policy.max_instances);
        self.current_instances.store(clamped, Ordering::Relaxed);
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

    #[tokio::test]
    async fn scale_up_on_high_cpu() {
        let scaler = AutoScaler::new(ScalePolicy {
            min_instances: 1,
            max_instances: 5,
            cooldown_secs: 0,
            ..Default::default()
        });

        assert_eq!(scaler.instances(), 1);
        let event = scaler.evaluate(80, 50, 10.0).await;
        assert!(event.is_some());
        assert_eq!(scaler.instances(), 2);
    }

    #[tokio::test]
    async fn scale_down_on_low_cpu() {
        let scaler = AutoScaler::new(ScalePolicy {
            min_instances: 1,
            max_instances: 5,
            cooldown_secs: 0,
            ..Default::default()
        });

        scaler.set_instances(3);
        let event = scaler.evaluate(20, 30, 10.0).await;
        assert!(event.is_some());
        assert_eq!(scaler.instances(), 2);
    }

    #[tokio::test]
    async fn no_scale_in_range() {
        let scaler = AutoScaler::new(ScalePolicy {
            cooldown_secs: 0,
            ..Default::default()
        });

        let event = scaler.evaluate(50, 50, 10.0).await;
        assert!(event.is_none());
        assert_eq!(scaler.instances(), 1);
    }

    #[tokio::test]
    async fn cooldown_prevents_rapid_scaling() {
        let scaler = AutoScaler::new(ScalePolicy {
            cooldown_secs: 3600, // 1 hour cooldown
            ..Default::default()
        });

        scaler.evaluate(80, 50, 10.0).await; // scale up
        let event = scaler.evaluate(80, 50, 10.0).await; // should be blocked by cooldown
        assert!(event.is_none());
    }

    #[tokio::test]
    async fn manual_set_instances() {
        let scaler = AutoScaler::new(ScalePolicy {
            min_instances: 1,
            max_instances: 10,
            ..Default::default()
        });

        scaler.set_instances(5);
        assert_eq!(scaler.instances(), 5);

        scaler.set_instances(0); // clamped to min
        assert_eq!(scaler.instances(), 1);

        scaler.set_instances(20); // clamped to max
        assert_eq!(scaler.instances(), 10);
    }
}
