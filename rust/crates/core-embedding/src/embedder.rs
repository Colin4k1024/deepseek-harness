//! Embedding — text-to-vector conversion.

use serde::{Deserialize, Serialize};

/// Embedding configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    pub model: String,
    pub dimension: usize,
    pub max_tokens: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model: "text-embedding-3-small".into(),
            dimension: 1536,
            max_tokens: 8191,
        }
    }
}

/// Trait for text embedders.
pub trait Embedder: Send + Sync {
    /// Embed a single text.
    fn embed(&self, text: &str) -> Vec<f64>;

    /// Embed multiple texts (batch).
    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f64>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }
}

/// Simple hash-based embedder for testing (not production quality).
pub struct SimpleEmbedder {
    dimension: usize,
}

impl SimpleEmbedder {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

impl Embedder for SimpleEmbedder {
    fn embed(&self, text: &str) -> Vec<f64> {
        let mut vec = vec![0.0; self.dimension];
        for (i, byte) in text.bytes().enumerate() {
            vec[i % self.dimension] += byte as f64 / 255.0;
        }
        // Normalize
        let norm: f64 = vec.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm > 0.0 {
            vec.iter_mut().for_each(|x| *x /= norm);
        }
        vec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_produces_vector() {
        let embedder = SimpleEmbedder::new(128);
        let vec = embedder.embed("hello world");
        assert_eq!(vec.len(), 128);
        // Should be normalized
        let norm: f64 = vec.iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!((norm - 1.0).abs() < 1e-10);
    }

    #[test]
    fn similar_texts_produce_similar_vectors() {
        let embedder = SimpleEmbedder::new(128);
        let v1 = embedder.embed("hello world");
        let v2 = embedder.embed("hello world!");
        let v3 = embedder.embed("completely different");

        let sim_12: f64 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
        let sim_13: f64 = v1.iter().zip(v3.iter()).map(|(a, b)| a * b).sum();
        assert!(sim_12 > sim_13); // similar texts more similar than different
    }
}
