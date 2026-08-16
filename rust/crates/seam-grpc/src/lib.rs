//! gRPC communication — service mesh, service discovery, and load balancing.

pub mod client;
pub mod mesh;
pub mod service;

pub use client::{GrpcClient, GrpcClientConfig, RpcResult};
pub use mesh::{MeshConfig, ServiceEntry, ServiceMesh};
pub use service::{GrpcMetadata, GrpcService, MethodDef};
