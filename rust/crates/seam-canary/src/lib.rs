//! Canary release — gradual rollout mechanism.

pub mod canary;
pub mod traffic;

pub use canary::{CanaryConfig, CanaryDeployment, CanaryState};
pub use traffic::{TrafficRouter, TrafficSplit};
