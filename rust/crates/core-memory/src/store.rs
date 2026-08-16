//! MemoryStore — trait and in-memory implementation for agent memory.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::entry::MemoryEntry;

/// Trait for memory storage backends.
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Store a memory entry.
    async fn store(&self, entry: MemoryEntry) -> Result<String, MemoryError>;

    /// Retrieve a memory by ID.
    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>, MemoryError>;

    /// Update an existing memory entry.
    async fn update(&self, entry: MemoryEntry) -> Result<(), MemoryError>;

    /// Delete a memory by ID.
    async fn delete(&self, id: &str) -> Result<(), MemoryError>;

    /// Search memories by text query (simple keyword match).
    async fn search_text(&self, query: &str, limit: usize)
    -> Result<Vec<MemoryEntry>, MemoryError>;

    /// List all memories, optionally filtered by type.
    async fn list(
        &self,
        memory_type: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, MemoryError>;

    /// Get count of stored memories.
    async fn count(&self) -> Result<usize, MemoryError>;
}

/// Memory store errors.
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("storage error: {0}")]
    Storage(String),
    #[error("memory not found: {0}")]
    NotFound(String),
}

/// In-memory implementation of MemoryStore.
///
/// Suitable for development and testing. For production, use a persistent backend.
pub struct InMemoryStore {
    entries: RwLock<HashMap<String, MemoryEntry>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MemoryStore for InMemoryStore {
    async fn store(&self, entry: MemoryEntry) -> Result<String, MemoryError> {
        let id = entry.id.clone();
        self.entries.write().unwrap().insert(id.clone(), entry);
        Ok(id)
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>, MemoryError> {
        let mut entries = self.entries.write().unwrap();
        if let Some(entry) = entries.get_mut(id) {
            entry.record_access();
            Ok(Some(entry.clone()))
        } else {
            Ok(None)
        }
    }

    async fn update(&self, entry: MemoryEntry) -> Result<(), MemoryError> {
        let mut entries = self.entries.write().unwrap();
        if entries.contains_key(&entry.id) {
            entries.insert(entry.id.clone(), entry);
            Ok(())
        } else {
            Err(MemoryError::NotFound(entry.id))
        }
    }

    async fn delete(&self, id: &str) -> Result<(), MemoryError> {
        self.entries.write().unwrap().remove(id);
        Ok(())
    }

    async fn search_text(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let query_lower = query.to_lowercase();
        let entries = self.entries.read().unwrap();

        let mut matches: Vec<(f64, &MemoryEntry)> = entries
            .values()
            .filter_map(|entry| {
                let content_lower = entry.content.to_lowercase();
                if content_lower.contains(&query_lower) {
                    // Simple relevance: importance + keyword match position
                    let position_score = content_lower
                        .find(&query_lower)
                        .map(|p| 1.0 - (p as f64 / content_lower.len() as f64))
                        .unwrap_or(0.0);
                    let score = entry.importance * 0.5 + position_score * 0.5;
                    Some((score, entry))
                } else {
                    None
                }
            })
            .collect();

        matches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(matches
            .into_iter()
            .take(limit)
            .map(|(_, e)| e.clone())
            .collect())
    }

    async fn list(
        &self,
        memory_type: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let entries = self.entries.read().unwrap();
        let filtered: Vec<MemoryEntry> = entries
            .values()
            .filter(|e| memory_type.is_none_or(|t| e.memory_type == t))
            .take(limit)
            .cloned()
            .collect();
        Ok(filtered)
    }

    async fn count(&self) -> Result<usize, MemoryError> {
        Ok(self.entries.read().unwrap().len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn store_and_get() {
        let store = InMemoryStore::new();
        let entry = MemoryEntry::new("user likes Rust", "preference");
        let id = store.store(entry).await.unwrap();

        let retrieved = store.get(&id).await.unwrap().unwrap();
        assert_eq!(retrieved.content, "user likes Rust");
        assert_eq!(retrieved.access_count, 1); // get records access
    }

    #[tokio::test]
    async fn search_text() {
        let store = InMemoryStore::new();
        store
            .store(MemoryEntry::new("user prefers dark mode", "preference"))
            .await
            .unwrap();
        store
            .store(MemoryEntry::new(
                "user likes Rust programming",
                "preference",
            ))
            .await
            .unwrap();
        store
            .store(MemoryEntry::new("project uses PostgreSQL", "fact"))
            .await
            .unwrap();

        let results = store.search_text("dark", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("dark mode"));
    }

    #[tokio::test]
    async fn list_by_type() {
        let store = InMemoryStore::new();
        store
            .store(MemoryEntry::new("fact 1", "fact"))
            .await
            .unwrap();
        store
            .store(MemoryEntry::new("pref 1", "preference"))
            .await
            .unwrap();
        store
            .store(MemoryEntry::new("fact 2", "fact"))
            .await
            .unwrap();

        let all = store.list(None, 100).await.unwrap();
        assert_eq!(all.len(), 3);

        let facts = store.list(Some("fact"), 100).await.unwrap();
        assert_eq!(facts.len(), 2);
    }

    #[tokio::test]
    async fn delete() {
        let store = InMemoryStore::new();
        let id = store.store(MemoryEntry::new("test", "fact")).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 1);

        store.delete(&id).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn update() {
        let store = InMemoryStore::new();
        let mut entry = MemoryEntry::new("original", "fact");
        let id = store.store(entry.clone()).await.unwrap();

        entry.content = "updated".into();
        store.update(entry).await.unwrap();

        let retrieved = store.get(&id).await.unwrap().unwrap();
        assert_eq!(retrieved.content, "updated");
    }
}
