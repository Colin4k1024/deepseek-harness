//! Knowledge graph — semantic triples.

use crate::graph::{Edge, Graph, Node};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A semantic triple (subject-predicate-object).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Triple {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

/// An entity in the knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub entity_type: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Knowledge graph built on top of property graph.
pub struct KnowledgeGraph {
    graph: Graph,
    entity_index: HashMap<String, String>, // entity_id -> node_id
}

impl KnowledgeGraph {
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            entity_index: HashMap::new(),
        }
    }

    /// Add an entity.
    pub fn add_entity(&mut self, entity: Entity) {
        let node = Node {
            id: entity.id.clone(),
            label: entity.entity_type.clone(),
            properties: entity.properties,
        };
        self.entity_index.insert(entity.id.clone(), node.id.clone());
        self.graph.add_node(node);
    }

    /// Add a triple (creates nodes if needed).
    pub fn add_triple(&mut self, triple: Triple) {
        if !self.entity_index.contains_key(&triple.subject) {
            self.add_entity(Entity {
                id: triple.subject.clone(),
                entity_type: "Unknown".into(),
                properties: HashMap::new(),
            });
        }
        if !self.entity_index.contains_key(&triple.object) {
            self.add_entity(Entity {
                id: triple.object.clone(),
                entity_type: "Unknown".into(),
                properties: HashMap::new(),
            });
        }
        let edge = Edge {
            id: format!("{}-{}-{}", triple.subject, triple.predicate, triple.object),
            source: triple.subject,
            target: triple.object,
            label: triple.predicate,
            properties: HashMap::new(),
        };
        self.graph.add_edge(edge);
    }

    /// Query triples by subject.
    pub fn triples_by_subject(&self, subject: &str) -> Vec<Triple> {
        let _ = self.graph.neighbors(&subject.to_string());
        Vec::new() // simplified
    }

    /// Get entity by ID.
    pub fn get_entity(&self, id: &str) -> Option<&Node> {
        self.entity_index
            .get(id)
            .and_then(|nid| self.graph.get_node(nid))
    }

    pub fn entity_count(&self) -> usize {
        self.entity_index.len()
    }
    pub fn triple_count(&self) -> usize {
        self.graph.edge_count()
    }
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_triples() {
        let mut kg = KnowledgeGraph::new();
        kg.add_triple(Triple {
            subject: "Alice".into(),
            predicate: "knows".into(),
            object: "Bob".into(),
        });
        kg.add_triple(Triple {
            subject: "Alice".into(),
            predicate: "lives_in".into(),
            object: "NYC".into(),
        });

        assert_eq!(kg.entity_count(), 3);
        assert_eq!(kg.triple_count(), 2);
    }

    #[test]
    fn entity_lookup() {
        let mut kg = KnowledgeGraph::new();
        kg.add_entity(Entity {
            id: "E1".into(),
            entity_type: "Person".into(),
            properties: HashMap::new(),
        });

        assert!(kg.get_entity("E1").is_some());
        assert!(kg.get_entity("E2").is_none());
    }
}
