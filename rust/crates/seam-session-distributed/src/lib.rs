//! Distributed session store — Redis and Postgres backends for multi-node deployment.
//!
//! Provides the `DistributedSessionStore` trait that extends the basic
//! `SessionStore` with distributed features like locking, replication,
//! and cross-node session access.

pub mod store;

pub use store::{DistributedSessionConfig, DistributedSessionStore, SessionLock};
