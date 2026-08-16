//! Property graph implementation.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub type NodeId = String;
pub type EdgeId = String;

/// Graph node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub label: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Graph edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub source: NodeId,
    pub target: NodeId,
    pub label: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Property graph.
pub struct Graph {
    nodes: HashMap<NodeId, Node>,
    edges: HashMap<EdgeId, Edge>,
    adjacency: HashMap<NodeId, Vec<EdgeId>>,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            adjacency: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node: Node) {
        self.adjacency.entry(node.id.clone()).or_default();
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn add_edge(&mut self, edge: Edge) {
        self.adjacency
            .entry(edge.source.clone())
            .or_default()
            .push(edge.id.clone());
        self.edges.insert(edge.id.clone(), edge);
    }

    pub fn get_node(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }
    pub fn get_edge(&self, id: &EdgeId) -> Option<&Edge> {
        self.edges.get(id)
    }

    /// Get neighbors of a node.
    pub fn neighbors(&self, node_id: &NodeId) -> Vec<&Node> {
        self.adjacency
            .get(node_id)
            .map(|edge_ids| {
                edge_ids
                    .iter()
                    .filter_map(|eid| self.edges.get(eid))
                    .map(|e| &self.nodes[&e.target])
                    .collect()
            })
            .unwrap_or_default()
    }

    /// BFS traversal from a start node.
    pub fn bfs(&self, start: &NodeId) -> Vec<&Node> {
        let mut visited = HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut result = Vec::new();

        if self.nodes.contains_key(start) {
            queue.push_back(start.clone());
            visited.insert(start.clone());
        }

        while let Some(id) = queue.pop_front() {
            if let Some(node) = self.nodes.get(&id) {
                result.push(node);
                for neighbor in self.neighbors(&id) {
                    if visited.insert(neighbor.id.clone()) {
                        queue.push_back(neighbor.id.clone());
                    }
                }
            }
        }
        result
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, label: &str) -> Node {
        Node {
            id: id.into(),
            label: label.into(),
            properties: HashMap::new(),
        }
    }

    fn edge(id: &str, src: &str, tgt: &str, label: &str) -> Edge {
        Edge {
            id: id.into(),
            source: src.into(),
            target: tgt.into(),
            label: label.into(),
            properties: HashMap::new(),
        }
    }

    #[test]
    fn add_and_query() {
        let mut g = Graph::new();
        g.add_node(node("a", "Person"));
        g.add_node(node("b", "City"));
        g.add_edge(edge("e1", "a", "b", "lives_in"));

        assert_eq!(g.node_count(), 2);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.neighbors(&"a".into()).len(), 1);
    }

    #[test]
    fn bfs_traversal() {
        let mut g = Graph::new();
        g.add_node(node("a", "A"));
        g.add_node(node("b", "B"));
        g.add_node(node("c", "C"));
        g.add_edge(edge("e1", "a", "b", "to"));
        g.add_edge(edge("e2", "b", "c", "to"));

        let result = g.bfs(&"a".into());
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].id, "a");
        assert_eq!(result[1].id, "b");
        assert_eq!(result[2].id, "c");
    }
}
