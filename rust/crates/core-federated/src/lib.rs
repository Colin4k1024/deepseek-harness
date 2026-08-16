//! Federated Learning — model aggregation and gradient sync.

pub mod aggregation;
pub use aggregation::{FederatedRound, GradientUpdate, ModelAggregator};
