//! Service Mesh — service discovery, health checking, load balancing.

pub mod mesh;
pub use mesh::{MeshConfig, ServiceEntry, ServiceMesh};
