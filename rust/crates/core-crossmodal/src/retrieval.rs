//! Cross-modal retrieval — search across modalities.

use dsh_core_multimodal::{Modality, MultimodalContent};
use dsh_core_vectordb::{VectorDb, VectorEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cross-modal search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossModalResult {
    pub content_id: String,
    pub modality: Modality,
    pub score: f64,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Cross-modal index — stores embeddings from multiple modalities.
pub struct CrossModalIndex {
    db: VectorDb,
    modality_map: HashMap<String, Modality>,
}

impl CrossModalIndex {
    pub fn new(dimension: usize) -> Self {
        Self {
            db: VectorDb::new(dimension),
            modality_map: HashMap::new(),
        }
    }

    /// Index content with an embedding.
    pub fn index(
        &mut self,
        content: &MultimodalContent,
        embedding: Vec<f64>,
    ) -> Result<(), String> {
        let mut metadata = HashMap::new();
        metadata.insert("modality".into(), serde_json::json!(content.modality));
        metadata.insert(
            "mime_type".into(),
            serde_json::json!(content.metadata.mime_type),
        );

        self.modality_map
            .insert(content.id.clone(), content.modality.clone());
        self.db.insert(VectorEntry {
            id: content.id.clone(),
            vector: embedding,
            metadata,
        })
    }

    /// Search across all modalities.
    pub fn search(&self, query_embedding: &[f64], k: usize) -> Vec<CrossModalResult> {
        self.db
            .search(query_embedding, k)
            .iter()
            .map(|r| {
                let modality = r
                    .entry
                    .metadata
                    .get("modality")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or(Modality::Text);
                CrossModalResult {
                    content_id: r.entry.id.clone(),
                    modality,
                    score: r.score,
                    metadata: r.entry.metadata.clone(),
                }
            })
            .collect()
    }

    /// Search within a specific modality.
    pub fn search_modality(
        &self,
        query_embedding: &[f64],
        k: usize,
        modality: &Modality,
    ) -> Vec<CrossModalResult> {
        self.search(query_embedding, k * 3)
            .into_iter()
            .filter(|r| &r.modality == modality)
            .take(k)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.db.len()
    }
    pub fn is_empty(&self) -> bool {
        self.db.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_and_search() {
        let mut index = CrossModalIndex::new(4);

        let text = MultimodalContent::text("hello");
        index.index(&text, vec![1.0, 0.0, 0.0, 0.0]).unwrap();

        let img = MultimodalContent::image(vec![0; 10], 2, 2);
        index.index(&img, vec![0.0, 1.0, 0.0, 0.0]).unwrap();

        let results = index.search(&[1.0, 0.0, 0.0, 0.0], 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].content_id, text.id);
    }

    #[test]
    fn search_by_modality() {
        let mut index = CrossModalIndex::new(4);

        let text = MultimodalContent::text("hello");
        index.index(&text, vec![1.0, 0.0, 0.0, 0.0]).unwrap();

        let img = MultimodalContent::image(vec![0; 10], 2, 2);
        index.index(&img, vec![0.9, 0.1, 0.0, 0.0]).unwrap();

        let results = index.search_modality(&[1.0, 0.0, 0.0, 0.0], 5, &Modality::Image);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].modality, Modality::Image);
    }
}
