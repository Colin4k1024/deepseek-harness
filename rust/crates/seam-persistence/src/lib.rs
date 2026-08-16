//! L2: Session persistence seam.
//!
//! Provides a `SessionStore` trait for durable session log storage,
//! with JSONL and SQLite backend implementations.

pub mod jsonl;
pub mod sqlite;
pub mod trait_def;

pub use jsonl::JsonlStore;
pub use sqlite::SqliteStore;
pub use trait_def::{SessionMetadata, SessionStore, StoreError};
