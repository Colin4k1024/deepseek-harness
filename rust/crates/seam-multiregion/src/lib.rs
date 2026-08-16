//! Multi-region deployment — region-aware routing and failover.

pub mod region;
pub use region::{Region, RegionConfig, RegionRouter};
