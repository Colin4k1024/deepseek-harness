//! Edge computing — cluster registry, edge nodes, and distributed routing.

pub mod cluster;
pub mod node;

pub use cluster::{Cluster, ClusterConfig, ClusterRegistry};
pub use node::{EdgeNode, EdgeRegistry};
