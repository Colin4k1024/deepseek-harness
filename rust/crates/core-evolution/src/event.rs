//! EvolutionEvent — append-only event log for agent evolution tracking.
//!
//! Tracks the lifecycle of genes and capsules: creation, evolution,
//! verification, scoring, and archival.

use serde::{Deserialize, Serialize};

/// An event in the agent evolution timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EvolutionEvent {
    /// A new gene was created.
    GeneCreated {
        gene_id: String,
        name: String,
        timestamp: String,
    },

    /// A gene was evolved (mutated) into a child.
    GeneEvolved {
        parent_id: String,
        child_id: String,
        timestamp: String,
    },

    /// A gene's confidence was updated based on outcome.
    GeneScored {
        gene_id: String,
        outcome: f64,
        new_confidence: f64,
        timestamp: String,
    },

    /// A capsule was created from a session.
    CapsuleCreated {
        capsule_id: String,
        gene_id: String,
        outcome: f64,
        timestamp: String,
    },

    /// A capsule was verified (replayed successfully).
    CapsuleVerified {
        capsule_id: String,
        timestamp: String,
    },

    /// A capsule was archived (no longer actively used).
    CapsuleArchived {
        capsule_id: String,
        reason: String,
        timestamp: String,
    },
}

impl EvolutionEvent {
    /// Get the timestamp of this event.
    pub fn timestamp(&self) -> &str {
        match self {
            Self::GeneCreated { timestamp, .. } => timestamp,
            Self::GeneEvolved { timestamp, .. } => timestamp,
            Self::GeneScored { timestamp, .. } => timestamp,
            Self::CapsuleCreated { timestamp, .. } => timestamp,
            Self::CapsuleVerified { timestamp, .. } => timestamp,
            Self::CapsuleArchived { timestamp, .. } => timestamp,
        }
    }
}

/// Append-only evolution log.
pub struct EvolutionLog {
    events: Vec<EvolutionEvent>,
}

impl EvolutionLog {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Append an event to the log.
    pub fn push(&mut self, event: EvolutionEvent) {
        self.events.push(event);
    }

    /// Get all events.
    pub fn events(&self) -> &[EvolutionEvent] {
        &self.events
    }

    /// Get events for a specific gene.
    pub fn events_for_gene(&self, gene_id: &str) -> Vec<&EvolutionEvent> {
        self.events
            .iter()
            .filter(|e| match e {
                EvolutionEvent::GeneCreated { gene_id: id, .. } => id == gene_id,
                EvolutionEvent::GeneEvolved {
                    parent_id,
                    child_id,
                    ..
                } => parent_id == gene_id || child_id == gene_id,
                EvolutionEvent::GeneScored { gene_id: id, .. } => id == gene_id,
                EvolutionEvent::CapsuleCreated { gene_id: id, .. } => id == gene_id,
                _ => false,
            })
            .collect()
    }

    /// Number of events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl Default for EvolutionLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> String {
        "1234567890".into()
    }

    #[test]
    fn log_push_and_query() {
        let mut log = EvolutionLog::new();
        log.push(EvolutionEvent::GeneCreated {
            gene_id: "g1".into(),
            name: "test".into(),
            timestamp: ts(),
        });
        log.push(EvolutionEvent::GeneEvolved {
            parent_id: "g1".into(),
            child_id: "g2".into(),
            timestamp: ts(),
        });
        log.push(EvolutionEvent::CapsuleCreated {
            capsule_id: "c1".into(),
            gene_id: "g1".into(),
            outcome: 0.9,
            timestamp: ts(),
        });

        assert_eq!(log.len(), 3);
        assert_eq!(log.events_for_gene("g1").len(), 3); // created, evolved (as parent), capsule
        assert_eq!(log.events_for_gene("g2").len(), 1); // evolved (as child)
    }

    #[test]
    fn event_timestamp() {
        let event = EvolutionEvent::GeneCreated {
            gene_id: "g1".into(),
            name: "test".into(),
            timestamp: "2026-08-15T00:00:00Z".into(),
        };
        assert_eq!(event.timestamp(), "2026-08-15T00:00:00Z");
    }
}
