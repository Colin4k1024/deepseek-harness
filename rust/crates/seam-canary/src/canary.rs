//! Canary deployment — gradual rollout with health checks.

use serde::{Deserialize, Serialize};

/// Canary configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryConfig {
    /// Name of the canary deployment.
    pub name: String,

    /// Initial traffic percentage for canary (0-100).
    pub initial_percent: u32,

    /// Increment per step.
    pub step_percent: u32,

    /// Health check interval in seconds.
    pub health_check_interval_secs: u64,

    /// Maximum error rate before rollback (0.0 - 1.0).
    pub max_error_rate: f64,

    /// Minimum requests before evaluating health.
    pub min_requests: u32,
}

impl Default for CanaryConfig {
    fn default() -> Self {
        Self {
            name: "default".into(),
            initial_percent: 5,
            step_percent: 10,
            health_check_interval_secs: 60,
            max_error_rate: 0.05,
            min_requests: 100,
        }
    }
}

/// Canary deployment state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CanaryState {
    /// Not started.
    Pending,
    /// Rolling out.
    Active,
    /// Fully promoted to production.
    Promoted,
    /// Rolled back due to issues.
    RolledBack,
    /// Paused by operator.
    Paused,
}

/// Canary deployment — tracks gradual rollout status.
pub struct CanaryDeployment {
    config: CanaryConfig,
    state: CanaryState,
    current_percent: u32,
    total_requests: u64,
    failed_requests: u64,
}

impl CanaryDeployment {
    /// Create a new canary deployment.
    pub fn new(config: CanaryConfig) -> Self {
        let initial_percent = config.initial_percent;
        Self {
            config,
            state: CanaryState::Pending,
            current_percent: initial_percent,
            total_requests: 0,
            failed_requests: 0,
        }
    }

    /// Start the canary deployment.
    pub fn start(&mut self) {
        self.state = CanaryState::Active;
        self.current_percent = self.config.initial_percent;
    }

    /// Get the current state.
    pub fn state(&self) -> CanaryState {
        self.state
    }

    /// Get the current traffic percentage for canary.
    pub fn current_percent(&self) -> u32 {
        self.current_percent
    }

    /// Record a request result.
    pub fn record_request(&mut self, success: bool) {
        self.total_requests += 1;
        if !success {
            self.failed_requests += 1;
        }
    }

    /// Get the current error rate.
    pub fn error_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        self.failed_requests as f64 / self.total_requests as f64
    }

    /// Check if health is acceptable.
    pub fn is_healthy(&self) -> bool {
        if self.total_requests < self.config.min_requests as u64 {
            return true; // Not enough data
        }
        self.error_rate() <= self.config.max_error_rate
    }

    /// Advance to the next step (increase traffic).
    pub fn advance(&mut self) -> bool {
        if self.state != CanaryState::Active {
            return false;
        }

        if !self.is_healthy() {
            self.state = CanaryState::RolledBack;
            return false;
        }

        self.current_percent = (self.current_percent + self.config.step_percent).min(100);

        if self.current_percent >= 100 {
            self.state = CanaryState::Promoted;
        }

        // Reset counters for next step
        self.total_requests = 0;
        self.failed_requests = 0;

        true
    }

    /// Rollback the canary.
    pub fn rollback(&mut self) {
        self.state = CanaryState::RolledBack;
        self.current_percent = 0;
    }

    /// Pause the canary.
    pub fn pause(&mut self) {
        if self.state == CanaryState::Active {
            self.state = CanaryState::Paused;
        }
    }

    /// Resume the canary.
    pub fn resume(&mut self) {
        if self.state == CanaryState::Paused {
            self.state = CanaryState::Active;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_deployment() {
        let canary = CanaryDeployment::new(CanaryConfig::default());
        assert_eq!(canary.state(), CanaryState::Pending);
        assert_eq!(canary.current_percent(), 5);
    }

    #[test]
    fn start_and_advance() {
        let mut canary = CanaryDeployment::new(CanaryConfig {
            initial_percent: 10,
            step_percent: 20,
            min_requests: 1,
            ..Default::default()
        });

        canary.start();
        assert_eq!(canary.state(), CanaryState::Active);
        assert_eq!(canary.current_percent(), 10);

        canary.record_request(true);
        canary.advance();
        assert_eq!(canary.current_percent(), 30);
    }

    #[test]
    fn unhealthy_rollback() {
        let mut canary = CanaryDeployment::new(CanaryConfig {
            name: "test".into(),
            initial_percent: 10,
            step_percent: 20,
            health_check_interval_secs: 60,
            max_error_rate: 0.1,
            min_requests: 5,
        });

        canary.start();

        // Record 5 failures out of 5 requests (100% error rate)
        for _ in 0..5 {
            canary.record_request(false);
        }

        canary.advance();
        assert_eq!(canary.state(), CanaryState::RolledBack);
    }

    #[test]
    fn promote_at_100() {
        let mut canary = CanaryDeployment::new(CanaryConfig {
            initial_percent: 50,
            step_percent: 50,
            min_requests: 1,
            ..Default::default()
        });

        canary.start();
        canary.record_request(true);
        canary.advance();
        assert_eq!(canary.state(), CanaryState::Promoted);
        assert_eq!(canary.current_percent(), 100);
    }

    #[test]
    fn pause_and_resume() {
        let mut canary = CanaryDeployment::new(CanaryConfig::default());
        canary.start();

        canary.pause();
        assert_eq!(canary.state(), CanaryState::Paused);

        canary.resume();
        assert_eq!(canary.state(), CanaryState::Active);
    }

    #[test]
    fn rollback() {
        let mut canary = CanaryDeployment::new(CanaryConfig::default());
        canary.start();
        canary.rollback();

        assert_eq!(canary.state(), CanaryState::RolledBack);
        assert_eq!(canary.current_percent(), 0);
    }
}
