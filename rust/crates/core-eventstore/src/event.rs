//! Domain event types.

use serde::{Deserialize, Serialize};

/// Unique event identifier.
pub type EventId = String;

/// An envelope wrapping a domain event with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Unique event ID.
    pub id: EventId,

    /// Aggregate/entity ID this event belongs to.
    pub aggregate_id: String,

    /// Sequence number within the aggregate.
    pub sequence: u64,

    /// Event type name.
    pub event_type: String,

    /// Event payload as JSON.
    pub payload: serde_json::Value,

    /// Timestamp (Unix seconds).
    pub timestamp: u64,
}

impl EventEnvelope {
    /// Create a new event envelope.
    pub fn new(
        aggregate_id: impl Into<String>,
        event_type: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            aggregate_id: aggregate_id.into(),
            sequence: 0,
            event_type: event_type.into(),
            payload,
            timestamp: now(),
        }
    }

    /// Set the sequence number.
    pub fn with_sequence(mut self, seq: u64) -> Self {
        self.sequence = seq;
        self
    }
}

/// Trait for domain events.
pub trait DomainEvent: Send + Sync {
    /// Event type name.
    fn event_type(&self) -> &str;

    /// Serialize to JSON payload.
    fn to_json(&self) -> serde_json::Value;
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_envelope() {
        let env = EventEnvelope::new("agg-1", "user_created", serde_json::json!({"name": "test"}));
        assert_eq!(env.aggregate_id, "agg-1");
        assert_eq!(env.event_type, "user_created");
    }
}
