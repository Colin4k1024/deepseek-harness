//! Capsule — verified experience snapshot.
//!
//! A Capsule captures a successful agent interaction that can be
//! replayed, shared, or used as a reference for future behavior.
//! Think of it as a "best practice" extracted from experience.

use dsh_core_session::SessionEvent;
use serde::{Deserialize, Serialize};

/// A verified experience snapshot.
///
/// Capsules capture the complete context of a successful interaction:
/// - The gene (strategy) that was used
/// - The session log (what happened)
/// - The outcome score (how well it went)
/// - Metadata about the scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capsule {
    /// Unique identifier.
    pub id: String,

    /// The gene that produced this capsule.
    pub gene_id: String,

    /// Human-readable description of the scenario.
    pub scenario: String,

    /// The task/prompt that was given.
    pub task: String,

    /// The session log from the interaction.
    pub session_log: Vec<SessionEvent>,

    /// Outcome score (0.0 - 1.0).
    pub outcome: f64,

    /// Whether this capsule has been verified (replayed successfully).
    pub verified: bool,

    /// Tags for categorization.
    pub tags: Vec<String>,

    /// Creation timestamp (ISO 8601).
    pub created_at: String,
}

impl Capsule {
    /// Create a new unverified capsule from a session.
    pub fn new(
        gene_id: impl Into<String>,
        scenario: impl Into<String>,
        task: impl Into<String>,
        session_log: Vec<SessionEvent>,
        outcome: f64,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            gene_id: gene_id.into(),
            scenario: scenario.into(),
            task: task.into(),
            session_log,
            outcome,
            verified: false,
            tags: Vec::new(),
            created_at: chrono_now(),
        }
    }

    /// Mark this capsule as verified after successful replay.
    pub fn verify(&mut self) {
        self.verified = true;
    }

    /// Add a tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Number of events in the session log.
    pub fn event_count(&self) -> usize {
        self.session_log.len()
    }
}

/// Simple chrono-free timestamp (ISO 8601-like).
fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsh_core_session::SessionEvent;

    #[test]
    fn new_capsule() {
        let events = vec![
            SessionEvent::UserMessage {
                seq: 1, time: 1000, content: "hello".into(), turn: Some(1), source: None,
            },
            SessionEvent::AssistantMessage {
                seq: 2, time: 1001, turn: 1, step: 1, message: "hi".into(), usage: None, source_event_seqs: None,
            },
        ];

        let capsule = Capsule::new("gene-1", "greeting", "say hello", events, 0.9);
        assert_eq!(capsule.gene_id, "gene-1");
        assert_eq!(capsule.outcome, 0.9);
        assert!(!capsule.verified);
        assert_eq!(capsule.event_count(), 2);
    }

    #[test]
    fn verify_capsule() {
        let events = vec![SessionEvent::UserMessage {
            seq: 1, time: 1000, content: "test".into(), turn: Some(1), source: None,
        }];
        let mut capsule = Capsule::new("gene-1", "test", "do test", events, 0.8);
        assert!(!capsule.verified);
        capsule.verify();
        assert!(capsule.verified);
    }

    #[test]
    fn with_tag() {
        let events = vec![];
        let capsule = Capsule::new("gene-1", "test", "task", events, 0.7)
            .with_tag("tool-use")
            .with_tag("successful");
        assert_eq!(capsule.tags, vec!["tool-use", "successful"]);
    }
}
