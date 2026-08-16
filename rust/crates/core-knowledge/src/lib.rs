//! Knowledge graph — semantic triples, entity management, and reasoning.

pub mod knowledge;
pub mod reasoning;

pub use knowledge::{Entity, KnowledgeGraph, Triple};
pub use reasoning::{Inference, ReasoningEngine, Rule};
