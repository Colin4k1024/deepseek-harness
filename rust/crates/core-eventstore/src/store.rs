//! Event store — append-only event persistence.

use std::collections::HashMap;
use std::sync::RwLock;

use super::event::EventEnvelope;

/// Trait for event stores.
pub trait EventStore: Send + Sync {
    /// Append events to an aggregate's stream.
    fn append(&self, aggregate_id: &str, events: Vec<EventEnvelope>)
    -> Result<(), EventStoreError>;

    /// Load all events for an aggregate.
    fn load(&self, aggregate_id: &str) -> Result<Vec<EventEnvelope>, EventStoreError>;

    /// Load events for an aggregate starting from a sequence number.
    fn load_from(
        &self,
        aggregate_id: &str,
        from_sequence: u64,
    ) -> Result<Vec<EventEnvelope>, EventStoreError>;

    /// Get the current sequence number for an aggregate.
    fn current_sequence(&self, aggregate_id: &str) -> u64;
}

/// Event store errors.
#[derive(Debug, thiserror::Error)]
pub enum EventStoreError {
    #[error("concurrency conflict: expected sequence {expected}, got {actual}")]
    ConcurrencyConflict { expected: u64, actual: u64 },
    #[error("storage error: {0}")]
    Storage(String),
}

/// In-memory event store for testing.
pub struct InMemoryEventStore {
    streams: RwLock<HashMap<String, Vec<EventEnvelope>>>,
}

impl InMemoryEventStore {
    pub fn new() -> Self {
        Self {
            streams: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryEventStore {
    fn default() -> Self {
        Self::new()
    }
}

impl EventStore for InMemoryEventStore {
    fn append(
        &self,
        aggregate_id: &str,
        events: Vec<EventEnvelope>,
    ) -> Result<(), EventStoreError> {
        let mut streams = self.streams.write().unwrap();
        let stream = streams.entry(aggregate_id.to_string()).or_default();

        let start_seq = stream.len() as u64 + 1;
        for (i, mut event) in events.into_iter().enumerate() {
            event.sequence = start_seq + i as u64;
            stream.push(event);
        }
        Ok(())
    }

    fn load(&self, aggregate_id: &str) -> Result<Vec<EventEnvelope>, EventStoreError> {
        let streams = self.streams.read().unwrap();
        Ok(streams.get(aggregate_id).cloned().unwrap_or_default())
    }

    fn load_from(
        &self,
        aggregate_id: &str,
        from_sequence: u64,
    ) -> Result<Vec<EventEnvelope>, EventStoreError> {
        let streams = self.streams.read().unwrap();
        let events = streams.get(aggregate_id).cloned().unwrap_or_default();
        Ok(events
            .into_iter()
            .filter(|e| e.sequence >= from_sequence)
            .collect())
    }

    fn current_sequence(&self, aggregate_id: &str) -> u64 {
        let streams = self.streams.read().unwrap();
        streams
            .get(aggregate_id)
            .map(|s| s.len() as u64)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_load() {
        let store = InMemoryEventStore::new();
        let events = vec![
            EventEnvelope::new("agg-1", "created", serde_json::json!({})),
            EventEnvelope::new("agg-1", "updated", serde_json::json!({})),
        ];

        store.append("agg-1", events).unwrap();
        let loaded = store.load("agg-1").unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].sequence, 1);
        assert_eq!(loaded[1].sequence, 2);
    }

    #[test]
    fn load_from_sequence() {
        let store = InMemoryEventStore::new();
        let events = vec![
            EventEnvelope::new("agg-1", "e1", serde_json::json!({})),
            EventEnvelope::new("agg-1", "e2", serde_json::json!({})),
            EventEnvelope::new("agg-1", "e3", serde_json::json!({})),
        ];
        store.append("agg-1", events).unwrap();

        let from_seq2 = store.load_from("agg-1", 2).unwrap();
        assert_eq!(from_seq2.len(), 2);
    }

    #[test]
    fn current_sequence() {
        let store = InMemoryEventStore::new();
        assert_eq!(store.current_sequence("agg-1"), 0);

        store
            .append(
                "agg-1",
                vec![EventEnvelope::new("agg-1", "e1", serde_json::json!({}))],
            )
            .unwrap();
        assert_eq!(store.current_sequence("agg-1"), 1);
    }
}
