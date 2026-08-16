//! Multi-cluster management — cluster registry and cross-cluster routing.

pub mod cluster;
pub use cluster::{Cluster, ClusterConfig, ClusterRegistry};
