//! Gene — agent strategy/policy DNA.
//!
//! A Gene defines how an agent should behave: which LLM to use,
//! what tools are available, system prompt, temperature, etc.
//! Genes can be evolved (mutated) and scored based on outcomes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A Gene defines agent behavior parameters.
///
/// Genes are immutable once created. Evolution produces new Genes
/// rather than mutating existing ones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gene {
    /// Unique identifier.
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Gene version (incremented on evolution).
    pub version: u32,

    /// Parent gene ID (None for root genes).
    pub parent_id: Option<String>,

    /// System prompt that defines agent personality/behavior.
    pub system_prompt: String,

    /// LLM model to use.
    pub model: String,

    /// Temperature parameter (0.0 - 2.0).
    pub temperature: f32,

    /// Max tokens for responses.
    pub max_tokens: u32,

    /// Allowed tool names (empty = all tools).
    pub allowed_tools: Vec<String>,

    /// Custom metadata (arbitrary key-value pairs).
    pub metadata: HashMap<String, serde_json::Value>,

    /// Confidence score (0.0 - 1.0, starts at 0.5).
    pub confidence: f64,
}

impl Gene {
    /// Create a new root gene.
    pub fn new(name: impl Into<String>, system_prompt: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            version: 1,
            parent_id: None,
            system_prompt: system_prompt.into(),
            model: "deepseek-chat".into(),
            temperature: 0.7,
            max_tokens: 4096,
            allowed_tools: Vec::new(),
            metadata: HashMap::new(),
            confidence: 0.5,
        }
    }

    /// Evolve this gene with mutations, producing a new child gene.
    pub fn evolve(&self, mutations: GeneMutation) -> Self {
        let mut child = self.clone();
        child.id = uuid::Uuid::new_v4().to_string();
        child.parent_id = Some(self.id.clone());
        child.version = self.version + 1;

        if let Some(prompt) = mutations.system_prompt {
            child.system_prompt = prompt;
        }
        if let Some(model) = mutations.model {
            child.model = model;
        }
        if let Some(temp) = mutations.temperature {
            child.temperature = temp;
        }
        if let Some(tokens) = mutations.max_tokens {
            child.max_tokens = tokens;
        }
        if let Some(tools) = mutations.allowed_tools {
            child.allowed_tools = tools;
        }
        for (k, v) in mutations.metadata {
            child.metadata.insert(k, v);
        }

        child
    }

    /// Update confidence based on an outcome score.
    ///
    /// Uses a simple moving average: new_confidence = old * 0.8 + outcome * 0.2
    pub fn update_confidence(&mut self, outcome: f64) {
        self.confidence = self.confidence * 0.8 + outcome.clamp(0.0, 1.0) * 0.2;
    }
}

/// Mutations to apply when evolving a Gene.
#[derive(Debug, Clone, Default)]
pub struct GeneMutation {
    pub system_prompt: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub allowed_tools: Option<Vec<String>>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_gene_defaults() {
        let gene = Gene::new("test", "You are helpful");
        assert_eq!(gene.name, "test");
        assert_eq!(gene.version, 1);
        assert!(gene.parent_id.is_none());
        assert_eq!(gene.confidence, 0.5);
    }

    #[test]
    fn evolve_produces_child() {
        let parent = Gene::new("parent", "original");
        let child = parent.evolve(GeneMutation {
            system_prompt: Some("evolved".into()),
            temperature: Some(0.9),
            ..Default::default()
        });

        assert_eq!(child.version, 2);
        assert_eq!(child.parent_id, Some(parent.id.clone()));
        assert_eq!(child.system_prompt, "evolved");
        assert_eq!(child.temperature, 0.9);
    }

    #[test]
    fn confidence_update() {
        let mut gene = Gene::new("test", "prompt");
        assert_eq!(gene.confidence, 0.5);

        gene.update_confidence(1.0); // good outcome
        assert!(gene.confidence > 0.5);

        gene.update_confidence(0.0); // bad outcome
        assert!(gene.confidence < 0.6);
    }
}
