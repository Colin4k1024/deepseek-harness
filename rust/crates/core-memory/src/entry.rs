//! MemoryEntry — a single memory with content, metadata, and optional embedding.

use serde::{Deserialize, Serialize};

/// Type alias for vector embeddings.
pub type Embedding = Vec<f32>;

/// A single memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique memory ID.
    pub id: String,

    /// Memory content (text).
    pub content: String,

    /// Memory type (e.g., "fact", "preference", "interaction", "instruction").
    pub memory_type: String,

    /// Optional metadata (arbitrary key-value pairs).
    pub metadata: serde_json::Value,

    /// Optional vector embedding for semantic search.
    pub embedding: Option<Embedding>,

    /// Importance score (0.0 - 1.0, higher = more important).
    pub importance: f64,

    /// Access count (for recency/frequency tracking).
    pub access_count: u64,

    /// Creation timestamp (Unix seconds).
    pub created_at: u64,

    /// Last access timestamp (Unix seconds).
    pub last_accessed_at: u64,
}

impl MemoryEntry {
    /// Create a new memory entry.
    pub fn new(content: impl Into<String>, memory_type: impl Into<String>) -> Self {
        let now = now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            content: content.into(),
            memory_type: memory_type.into(),
            metadata: serde_json::json!({}),
            embedding: None,
            importance: 0.5,
            access_count: 0,
            created_at: now,
            last_accessed_at: now,
        }
    }

    /// Set the embedding.
    pub fn with_embedding(mut self, embedding: Embedding) -> Self {
        self.embedding = Some(embedding);
        self
    }

    /// Set importance score.
    pub fn with_importance(mut self, importance: f64) -> Self {
        self.importance = importance.clamp(0.0, 1.0);
        self
    }

    /// Set metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Record an access.
    pub fn record_access(&mut self) {
        self.access_count += 1;
        self.last_accessed_at = now();
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_entry() {
        let entry = MemoryEntry::new("user prefers dark mode", "preference");
        assert!(!entry.id.is_empty());
        assert_eq!(entry.content, "user prefers dark mode");
        assert_eq!(entry.memory_type, "preference");
        assert_eq!(entry.importance, 0.5);
        assert_eq!(entry.access_count, 0);
    }

    #[test]
    fn with_embedding() {
        let emb = vec![0.1, 0.2, 0.3];
        let entry = MemoryEntry::new("test", "fact").with_embedding(emb.clone());
        assert_eq!(entry.embedding, Some(emb));
    }

    #[test]
    fn record_access() {
        let mut entry = MemoryEntry::new("test", "fact");
        assert_eq!(entry.access_count, 0);
        entry.record_access();
        assert_eq!(entry.access_count, 1);
        entry.record_access();
        assert_eq!(entry.access_count, 2);
    }

    #[test]
    fn importance_clamped() {
        let entry = MemoryEntry::new("test", "fact").with_importance(1.5);
        assert_eq!(entry.importance, 1.0);

        let entry = MemoryEntry::new("test", "fact").with_importance(-0.5);
        assert_eq!(entry.importance, 0.0);
    }
}
