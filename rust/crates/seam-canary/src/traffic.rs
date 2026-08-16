//! Traffic routing — split traffic between stable and canary versions.

use serde::{Deserialize, Serialize};

/// Traffic split configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSplit {
    /// Percentage of traffic to canary (0-100).
    pub canary_percent: u32,
}

impl TrafficSplit {
    /// Create a new traffic split.
    pub fn new(canary_percent: u32) -> Self {
        Self {
            canary_percent: canary_percent.min(100),
        }
    }

    /// Create a 100% stable split.
    pub fn stable() -> Self {
        Self { canary_percent: 0 }
    }

    /// Create a 100% canary split.
    pub fn canary() -> Self {
        Self {
            canary_percent: 100,
        }
    }
}

impl Default for TrafficSplit {
    fn default() -> Self {
        Self::stable()
    }
}

/// Traffic router — routes requests to stable or canary.
pub struct TrafficRouter {
    split: TrafficSplit,
}

impl TrafficRouter {
    /// Create a new traffic router.
    pub fn new(split: TrafficSplit) -> Self {
        Self { split }
    }

    /// Update the traffic split.
    pub fn set_split(&mut self, split: TrafficSplit) {
        self.split = split;
    }

    /// Get the current split.
    pub fn split(&self) -> &TrafficSplit {
        &self.split
    }

    /// Route a request — returns true for canary, false for stable.
    pub fn should_route_to_canary(&self) -> bool {
        rand::random::<u32>() % 100 < self.split.canary_percent
    }
}

/// Request routing result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteTarget {
    /// Route to stable version.
    Stable,
    /// Route to canary version.
    Canary,
}

impl TrafficRouter {
    /// Route a request and return the target.
    pub fn route(&self) -> RouteTarget {
        if self.should_route_to_canary() {
            RouteTarget::Canary
        } else {
            RouteTarget::Stable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_split() {
        let router = TrafficRouter::new(TrafficSplit::stable());
        assert_eq!(router.split().canary_percent, 0);
    }

    #[test]
    fn canary_split() {
        let router = TrafficRouter::new(TrafficSplit::canary());
        assert_eq!(router.split().canary_percent, 100);
    }

    #[test]
    fn route_deterministic_with_seed() {
        // Note: rand is not seeded, so this test is probabilistic.
        // For 0% canary, should always route to stable.
        let router = TrafficRouter::new(TrafficSplit::stable());
        for _ in 0..100 {
            assert_eq!(router.route(), RouteTarget::Stable);
        }
    }

    #[test]
    fn update_split() {
        let mut router = TrafficRouter::new(TrafficSplit::stable());
        router.set_split(TrafficSplit::new(50));
        assert_eq!(router.split().canary_percent, 50);
    }
}
