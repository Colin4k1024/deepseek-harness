//! RAG pipeline — retrieval + context assembly.

use dsh_core_embedding::Embedder;
use dsh_core_vectordb::{VectorDb, VectorEntry};
use serde::{Deserialize, Serialize};

/// RAG configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagConfig {
    pub top_k: usize,
    pub min_score: f64,
    pub max_context_tokens: usize,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            top_k: 5,
            min_score: 0.5,
            max_context_tokens: 4000,
        }
    }
}

/// RAG result with retrieved context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagResult {
    pub query: String,
    pub contexts: Vec<RetrievedContext>,
    pub assembled_prompt: String,
}

/// A retrieved context chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedContext {
    pub id: String,
    pub content: String,
    pub score: f64,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

/// RAG pipeline — embed query, search vector DB, assemble context.
pub struct RagPipeline<'a> {
    db: &'a mut VectorDb,
    embedder: &'a dyn Embedder,
    config: RagConfig,
}

impl<'a> RagPipeline<'a> {
    pub fn new(db: &'a mut VectorDb, embedder: &'a dyn Embedder, config: RagConfig) -> Self {
        Self {
            db,
            embedder,
            config,
        }
    }

    /// Ingest a document: embed and store.
    pub fn ingest(
        &mut self,
        id: &str,
        content: &str,
        metadata: std::collections::HashMap<String, serde_json::Value>,
    ) {
        let vector = self.embedder.embed(content);
        let entry = VectorEntry {
            id: id.into(),
            vector,
            metadata,
        };
        let _ = self.db.insert(entry);
    }

    /// Query: embed the query, search, assemble context.
    pub fn query(&mut self, query: &str) -> RagResult {
        let query_vec = self.embedder.embed(query);
        let results = self.db.search(&query_vec, self.config.top_k);

        let contexts: Vec<RetrievedContext> = results
            .iter()
            .filter(|r| r.score >= self.config.min_score)
            .map(|r| {
                let content = r
                    .entry
                    .metadata
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                RetrievedContext {
                    id: r.entry.id.clone(),
                    content,
                    score: r.score,
                    metadata: r.entry.metadata.clone(),
                }
            })
            .collect();

        let assembled_prompt = self.assemble_prompt(query, &contexts);

        RagResult {
            query: query.into(),
            contexts,
            assembled_prompt,
        }
    }

    fn assemble_prompt(&self, query: &str, contexts: &[RetrievedContext]) -> String {
        let mut prompt = String::new();
        prompt.push_str("Context:\n");
        for (i, ctx) in contexts.iter().enumerate() {
            prompt.push_str(&format!("[{}] {}\n\n", i + 1, ctx.content));
        }
        prompt.push_str(&format!("Question: {}\n\nAnswer:", query));
        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsh_core_embedding::embedder::SimpleEmbedder;
    use dsh_core_vectordb::VectorDb;

    #[test]
    fn ingest_and_query() {
        let mut db = VectorDb::new(128);
        let embedder = SimpleEmbedder::new(128);

        {
            let mut pipeline = RagPipeline::new(&mut db, &embedder, RagConfig::default());
            let mut meta = std::collections::HashMap::new();
            meta.insert(
                "content".into(),
                serde_json::json!("Rust is a systems programming language."),
            );
            pipeline.ingest("doc1", "Rust is a systems programming language.", meta);

            let mut meta2 = std::collections::HashMap::new();
            meta2.insert(
                "content".into(),
                serde_json::json!("Python is a high-level language."),
            );
            pipeline.ingest("doc2", "Python is a high-level language.", meta2);
        }

        let mut pipeline = RagPipeline::new(&mut db, &embedder, RagConfig::default());
        let result = pipeline.query("What is Rust?");
        assert!(!result.contexts.is_empty());
        assert!(result.assembled_prompt.contains("Context:"));
    }
}
