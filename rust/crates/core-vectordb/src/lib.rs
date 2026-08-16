//! Vector database — vector storage, similarity search, embeddings, and online learning.

pub mod embedder;
pub mod learning;
pub mod vectordb;

pub use embedder::{Embedder, EmbeddingConfig, SimpleEmbedder};
pub use learning::{ModelUpdate, OnlineModel};
pub use vectordb::{SearchResult, VectorDb, VectorEntry};
