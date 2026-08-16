//! Agent memory — long-term memory with semantic search.
//!
//! Provides a `MemoryStore` trait for storing and retrieving agent memories,
//! with support for vector embeddings and cosine similarity search.

pub mod entry;
pub mod search;
pub mod store;

pub use entry::MemoryEntry;
pub use search::{SemanticSearch, cosine_similarity};
pub use store::{InMemoryStore, MemoryStore};
