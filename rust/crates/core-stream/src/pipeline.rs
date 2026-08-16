//! Stream pipeline — real-time event processing.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// A stream event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    pub id: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub timestamp: u64,
}

impl StreamEvent {
    pub fn new(event_type: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            event_type: event_type.into(),
            payload,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

/// Trait for stream processors.
#[async_trait]
pub trait Processor: Send + Sync {
    /// Process an event, optionally producing output events.
    async fn process(&self, event: &StreamEvent) -> Option<StreamEvent>;
}

/// Stream pipeline — processes events through a chain of processors.
pub struct StreamPipeline {
    processors: Vec<Box<dyn Processor>>,
    buffer: VecDeque<StreamEvent>,
    max_buffer: usize,
}

impl StreamPipeline {
    pub fn new(max_buffer: usize) -> Self {
        Self {
            processors: Vec::new(),
            buffer: VecDeque::new(),
            max_buffer,
        }
    }

    /// Add a processor to the pipeline.
    pub fn add_processor(&mut self, p: impl Processor + 'static) {
        self.processors.push(Box::new(p));
    }

    /// Ingest an event into the pipeline.
    pub fn ingest(&mut self, event: StreamEvent) {
        if self.buffer.len() >= self.max_buffer {
            self.buffer.pop_front();
        }
        self.buffer.push_back(event);
    }

    /// Drain processed events from the buffer.
    pub fn drain(&mut self) -> Vec<StreamEvent> {
        self.buffer.drain(..).collect()
    }

    /// Current buffer size.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingest_and_drain() {
        let mut pipeline = StreamPipeline::new(10);
        pipeline.ingest(StreamEvent::new("test", serde_json::json!({})));
        assert_eq!(pipeline.len(), 1);

        let events = pipeline.drain();
        assert_eq!(events.len(), 1);
        assert!(pipeline.is_empty());
    }

    #[test]
    fn buffer_overflow() {
        let mut pipeline = StreamPipeline::new(2);
        pipeline.ingest(StreamEvent::new("a", serde_json::json!({})));
        pipeline.ingest(StreamEvent::new("b", serde_json::json!({})));
        pipeline.ingest(StreamEvent::new("c", serde_json::json!({}))); // evicts "a"

        assert_eq!(pipeline.len(), 2);
        let events = pipeline.drain();
        assert_eq!(events[0].event_type, "b");
    }
}
