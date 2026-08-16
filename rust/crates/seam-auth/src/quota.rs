//! Quota management — daily/monthly usage limits.
//!
//! Tracks usage per user/key and enforces configurable limits.

use std::collections::HashMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

/// Quota configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaConfig {
    /// Maximum requests per day (0 = unlimited).
    pub daily_requests: u64,

    /// Maximum tokens per day (0 = unlimited).
    pub daily_tokens: u64,

    /// Maximum requests per month (0 = unlimited).
    pub monthly_requests: u64,

    /// Maximum tokens per month (0 = unlimited).
    pub monthly_tokens: u64,
}

impl Default for QuotaConfig {
    fn default() -> Self {
        Self {
            daily_requests: 1000,
            daily_tokens: 1_000_000,
            monthly_requests: 30_000,
            monthly_tokens: 30_000_000,
        }
    }
}

/// Usage tracking for a single user/key.
#[derive(Debug, Clone, Default)]
struct Usage {
    daily_requests: u64,
    daily_tokens: u64,
    monthly_requests: u64,
    monthly_tokens: u64,
    day_start: u64,
    month_start: u64,
}

/// Quota manager — tracks and enforces usage limits.
pub struct QuotaManager {
    config: QuotaConfig,
    usage: RwLock<HashMap<String, Usage>>,
}

impl QuotaManager {
    /// Create a new quota manager.
    pub fn new(config: QuotaConfig) -> Self {
        Self {
            config,
            usage: RwLock::new(HashMap::new()),
        }
    }

    /// Check if a request is within quota.
    pub fn check(&self, user_id: &str) -> bool {
        let mut usage = self.usage.write().unwrap();
        let entry = usage.entry(user_id.to_string()).or_default();

        self.maybe_reset(entry);

        if self.config.daily_requests > 0 && entry.daily_requests >= self.config.daily_requests {
            return false;
        }
        if self.config.monthly_requests > 0
            && entry.monthly_requests >= self.config.monthly_requests
        {
            return false;
        }

        true
    }

    /// Record usage (after a successful request).
    pub fn record(&self, user_id: &str, tokens: u64) {
        let mut usage = self.usage.write().unwrap();
        let entry = usage.entry(user_id.to_string()).or_default();

        self.maybe_reset(entry);

        entry.daily_requests += 1;
        entry.daily_tokens += tokens;
        entry.monthly_requests += 1;
        entry.monthly_tokens += tokens;
    }

    /// Get current usage for a user.
    pub fn get_usage(&self, user_id: &str) -> (u64, u64, u64, u64) {
        let mut usage = self.usage.write().unwrap();
        if let Some(entry) = usage.get_mut(user_id) {
            self.maybe_reset(entry);
            (
                entry.daily_requests,
                entry.daily_tokens,
                entry.monthly_requests,
                entry.monthly_tokens,
            )
        } else {
            (0, 0, 0, 0)
        }
    }

    /// Reset usage for a user (admin operation).
    pub fn reset(&self, user_id: &str) {
        self.usage.write().unwrap().remove(user_id);
    }

    /// Check if day/month has rolled over and reset counters.
    fn maybe_reset(&self, entry: &mut Usage) {
        let current_day = now() / 86400;
        let current_month = now() / (86400 * 30);

        if entry.day_start != current_day {
            entry.daily_requests = 0;
            entry.daily_tokens = 0;
            entry.day_start = current_day;
        }

        if entry.month_start != current_month {
            entry.monthly_requests = 0;
            entry.monthly_tokens = 0;
            entry.month_start = current_month;
        }
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

    #[test]
    fn within_quota() {
        let qm = QuotaManager::new(QuotaConfig {
            daily_requests: 5,
            daily_tokens: 1000,
            monthly_requests: 100,
            monthly_tokens: 10000,
        });

        for _ in 0..5 {
            assert!(qm.check("user1"));
            qm.record("user1", 100);
        }
    }

    #[test]
    fn quota_exceeded() {
        let qm = QuotaManager::new(QuotaConfig {
            daily_requests: 2,
            daily_tokens: 1000,
            monthly_requests: 100,
            monthly_tokens: 10000,
        });

        qm.record("user1", 100);
        qm.record("user1", 100);
        assert!(!qm.check("user1")); // daily limit reached
    }

    #[test]
    fn per_user_isolation() {
        let qm = QuotaManager::new(QuotaConfig {
            daily_requests: 1,
            daily_tokens: 1000,
            monthly_requests: 100,
            monthly_tokens: 10000,
        });

        qm.record("user1", 100);
        assert!(!qm.check("user1"));
        assert!(qm.check("user2")); // user2 has separate quota
    }

    #[test]
    fn usage_tracking() {
        let qm = QuotaManager::new(QuotaConfig::default());

        qm.record("user1", 500);
        qm.record("user1", 300);

        let (dr, dt, mr, mt) = qm.get_usage("user1");
        assert_eq!(dr, 2);
        assert_eq!(dt, 800);
        assert_eq!(mr, 2);
        assert_eq!(mt, 800);
    }

    #[test]
    fn unlimited_quota() {
        let qm = QuotaManager::new(QuotaConfig {
            daily_requests: 0,
            daily_tokens: 0,
            monthly_requests: 0,
            monthly_tokens: 0,
        });

        for _ in 0..1000 {
            assert!(qm.check("user1"));
            qm.record("user1", 100);
        }
    }
}
