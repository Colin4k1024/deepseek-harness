//! Incremental computation graph.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Node identifier.
pub type NodeId = String;

/// A computation node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputationNode {
    pub id: NodeId,
    pub value: serde_json::Value,
    pub dependencies: Vec<NodeId>,
    pub dirty: bool,
}

/// Computation graph — tracks dependencies and propagates changes.
pub struct ComputationGraph {
    nodes: HashMap<NodeId, ComputationNode>,
}

impl ComputationGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// Add a node.
    pub fn add_node(&mut self, node: ComputationNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Mark a node as dirty and propagate to dependents.
    pub fn mark_dirty(&mut self, node_id: &NodeId) {
        let mut to_dirty = vec![node_id.clone()];
        while let Some(id) = to_dirty.pop() {
            if let Some(node) = self.nodes.get_mut(&id)
                && !node.dirty
            {
                node.dirty = true;
                let dependents: Vec<NodeId> = self
                    .nodes
                    .values()
                    .filter(|n| n.dependencies.contains(&id))
                    .map(|n| n.id.clone())
                    .collect();
                to_dirty.extend(dependents);
            }
        }
    }

    /// Get dirty nodes.
    pub fn dirty_nodes(&self) -> Vec<&ComputationNode> {
        self.nodes.values().filter(|n| n.dirty).collect()
    }

    /// Mark a node as clean.
    pub fn mark_clean(&mut self, node_id: &NodeId) {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.dirty = false;
        }
    }

    pub fn get(&self, id: &NodeId) -> Option<&ComputationNode> {
        self.nodes.get(id)
    }
}

impl Default for ComputationGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, deps: Vec<&str>) -> ComputationNode {
        ComputationNode {
            id: id.into(),
            value: serde_json::json!(null),
            dependencies: deps.into_iter().map(String::from).collect(),
            dirty: false,
        }
    }

    #[test]
    fn dirty_propagation() {
        let mut graph = ComputationGraph::new();
        graph.add_node(node("a", vec![]));
        graph.add_node(node("b", vec!["a"]));
        graph.add_node(node("c", vec!["b"]));

        graph.mark_dirty(&"a".into());
        assert_eq!(graph.dirty_nodes().len(), 3); // a, b, c all dirty
    }

    #[test]
    fn mark_clean() {
        let mut graph = ComputationGraph::new();
        graph.add_node(node("a", vec![]));
        graph.mark_dirty(&"a".into());
        graph.mark_clean(&"a".into());
        assert!(graph.dirty_nodes().is_empty());
    }
}
