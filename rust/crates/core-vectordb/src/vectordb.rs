//! Vector database — storage and similarity search.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A vector entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEntry {
    pub id: String,
    pub vector: Vec<f64>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Search result with similarity score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub entry: VectorEntry,
    pub score: f64,
}

/// In-memory vector database.
pub struct VectorDb {
    entries: HashMap<String, VectorEntry>,
    dimension: usize,
}

impl VectorDb {
    pub fn new(dimension: usize) -> Self {
        Self {
            entries: HashMap::new(),
            dimension,
        }
    }

    /// Insert a vector entry.
    pub fn insert(&mut self, entry: VectorEntry) -> Result<(), String> {
        if entry.vector.len() != self.dimension {
            return Err(format!(
                "Expected dimension {}, got {}",
                self.dimension,
                entry.vector.len()
            ));
        }
        self.entries.insert(entry.id.clone(), entry);
        Ok(())
    }

    /// Search for k nearest neighbors using cosine similarity.
    pub fn search(&self, query: &[f64], k: usize) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = self
            .entries
            .values()
            .map(|entry| SearchResult {
                entry: entry.clone(),
                score: cosine_similarity(query, &entry.vector),
            })
            .collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(k);
        results
    }

    /// Delete an entry by ID.
    pub fn delete(&mut self, id: &str) -> bool {
        self.entries.remove(id).is_some()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, vector: Vec<f64>) -> VectorEntry {
        VectorEntry {
            id: id.into(),
            vector,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn insert_and_search() {
        let mut db = VectorDb::new(3);
        db.insert(entry("a", vec![1.0, 0.0, 0.0])).unwrap();
        db.insert(entry("b", vec![0.0, 1.0, 0.0])).unwrap();
        db.insert(entry("c", vec![0.9, 0.1, 0.0])).unwrap();

        let results = db.search(&[1.0, 0.0, 0.0], 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].entry.id, "a"); // most similar
    }

    #[test]
    fn dimension_mismatch() {
        let mut db = VectorDb::new(3);
        let result = db.insert(entry("a", vec![1.0, 0.0]));
        assert!(result.is_err());
    }
}
