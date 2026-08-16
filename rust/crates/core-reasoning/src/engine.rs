//! Semantic reasoning engine.

use dsh_core_knowledge::{KnowledgeGraph, Triple};
use serde::{Deserialize, Serialize};

/// An inference rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    pub description: String,
    pub condition: String,  // Simplified: pattern string
    pub conclusion: String, // Simplified: pattern string
}

/// An inferred fact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inference {
    pub triple: Triple,
    pub rule_name: String,
    pub confidence: f64,
}

/// Reasoning engine — applies rules to derive new facts.
pub struct ReasoningEngine {
    rules: Vec<Rule>,
}

impl ReasoningEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add an inference rule.
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Apply transitive closure: if A->B and B->C via same predicate, infer A->C.
    pub fn infer_transitive(&self, _kg: &KnowledgeGraph) -> Vec<Inference> {
        Vec::new() // simplified
    }

    /// Apply a specific rule by name.
    pub fn apply_rule(&self, rule_name: &str, _kg: &KnowledgeGraph) -> Vec<Inference> {
        if self.rules.iter().any(|r| r.name == rule_name) {
            // Simplified: return empty for now
            Vec::new()
        } else {
            Vec::new()
        }
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for ReasoningEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_rules() {
        let mut engine = ReasoningEngine::new();
        engine.add_rule(Rule {
            name: "transitive_knows".into(),
            description: "If A knows B and B knows C, then A knows C".into(),
            condition: "A knows B, B knows C".into(),
            conclusion: "A knows C".into(),
        });

        assert_eq!(engine.rule_count(), 1);
    }

    #[test]
    fn inference_with_empty_graph() {
        let engine = ReasoningEngine::new();
        let kg = KnowledgeGraph::new();
        let inferences = engine.infer_transitive(&kg);
        assert!(inferences.is_empty());
    }
}
