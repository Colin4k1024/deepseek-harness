//! Semantic search — vector similarity search with cosine similarity.
//!
//! Provides cosine similarity computation and a `SemanticSearch` wrapper
//! that combines vector search with the MemoryStore trait.

use crate::entry::MemoryEntry;
use crate::store::{MemoryError, MemoryStore};

/// Compute cosine similarity between two vectors.
///
/// Returns a value between -1.0 and 1.0. Higher = more similar.
/// Returns 0.0 if either vector is empty or they have different lengths.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Semantic search result with similarity score.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub entry: MemoryEntry,
    pub similarity: f32,
}

/// Semantic search wrapper that searches memories by vector similarity.
pub struct SemanticSearch;

impl SemanticSearch {
    /// Search for memories similar to the given embedding.
    ///
    /// Returns results sorted by similarity (highest first).
    pub async fn search(
        store: &dyn MemoryStore,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
    ) -> Result<Vec<SearchResult>, MemoryError> {
        let entries = store.list(None, 10_000).await?;

        let mut results: Vec<SearchResult> = entries
            .into_iter()
            .filter_map(|entry| {
                let embedding = entry.embedding.as_ref()?;
                let similarity = cosine_similarity(query_embedding, embedding);
                if similarity >= min_similarity {
                    Some(SearchResult { entry, similarity })
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_different_lengths() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_similarity_empty() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[tokio::test]
    async fn semantic_search_finds_similar() {
        use crate::store::InMemoryStore;

        let store = InMemoryStore::new();

        // Store entries with embeddings
        store
            .store(
                MemoryEntry::new("dark mode preference", "preference")
                    .with_embedding(vec![0.9, 0.1, 0.0]),
            )
            .await
            .unwrap();
        store
            .store(MemoryEntry::new("likes Rust", "preference").with_embedding(vec![0.1, 0.9, 0.0]))
            .await
            .unwrap();
        store
            .store(MemoryEntry::new("uses PostgreSQL", "fact").with_embedding(vec![0.0, 0.1, 0.9]))
            .await
            .unwrap();

        // Search for something similar to "dark mode"
        let query = vec![0.85, 0.15, 0.05];
        let results = SemanticSearch::search(&store, &query, 5, 0.5)
            .await
            .unwrap();

        assert!(!results.is_empty());
        assert!(results[0].entry.content.contains("dark mode"));
        assert!(results[0].similarity > 0.9);
    }
}
