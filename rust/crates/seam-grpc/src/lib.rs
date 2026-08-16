//! gRPC Communication — service definition, client, and server traits.

pub mod client;
pub mod service;
pub use client::GrpcClient;
pub use service::GrpcService;
