//! Knowledge graph — property graph, semantic triples, entity management, and reasoning.

pub mod graph;
pub mod knowledge;
pub mod reasoning;

pub use graph::{Edge, EdgeId, Graph, Node, NodeId};
pub use knowledge::{Entity, KnowledgeGraph, Triple};
pub use reasoning::{Inference, ReasoningEngine, Rule};
