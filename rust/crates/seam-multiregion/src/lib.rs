//! Multi-region deployment — region-aware routing, failover, and CDN.

pub mod cdn;
pub mod region;

pub use cdn::{CachePolicy, CdnConfig, CdnRouter};
pub use region::{Region, RegionConfig, RegionRouter};
