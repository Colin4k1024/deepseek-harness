//! Message Bus — pub/sub with topic-based routing.

use std::collections::HashMap;
use std::sync::RwLock;
use tokio::sync::broadcast;

use super::message::BusMessage;

/// Trait for message buses.
pub trait MessageBus: Send + Sync {
    /// Publish a message to a topic.
    fn publish(&self, message: BusMessage) -> Result<(), BusError>;

    /// Subscribe to a topic. Returns a receiver.
    fn subscribe(&self, topic: &str) -> Result<broadcast::Receiver<BusMessage>, BusError>;
}

/// Message bus errors.
#[derive(Debug, thiserror::Error)]
pub enum BusError {
    #[error("publish error: {0}")]
    Publish(String),
    #[error("subscribe error: {0}")]
    Subscribe(String),
}

/// In-memory message bus using broadcast channels.
pub struct InMemoryMessageBus {
    channels: RwLock<HashMap<String, broadcast::Sender<BusMessage>>>,
    capacity: usize,
}

impl InMemoryMessageBus {
    /// Create a new message bus.
    pub fn new(capacity: usize) -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            capacity,
        }
    }
}

impl Default for InMemoryMessageBus {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl MessageBus for InMemoryMessageBus {
    fn publish(&self, message: BusMessage) -> Result<(), BusError> {
        let mut channels = self.channels.write().unwrap();
        let sender = channels
            .entry(message.topic.clone())
            .or_insert_with(|| broadcast::channel(self.capacity).0);
        sender
            .send(message)
            .map_err(|e| BusError::Publish(e.to_string()))?;
        Ok(())
    }

    fn subscribe(&self, topic: &str) -> Result<broadcast::Receiver<BusMessage>, BusError> {
        let mut channels = self.channels.write().unwrap();
        let sender = channels
            .entry(topic.to_string())
            .or_insert_with(|| broadcast::channel(self.capacity).0);
        Ok(sender.subscribe())
    }
}

#[cfg(test)]
mod tests {
    use super::super::message::MessageType;
    use super::*;

    #[tokio::test]
    async fn publish_and_subscribe() {
        let bus = InMemoryMessageBus::default();
        let mut rx = bus.subscribe("test").unwrap();

        bus.publish(BusMessage::new(
            "test",
            MessageType::Event,
            serde_json::json!({"a": 1}),
        ))
        .unwrap();

        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.topic, "test");
        assert_eq!(msg.payload["a"], 1);
    }

    #[tokio::test]
    async fn multiple_subscribers() {
        let bus = InMemoryMessageBus::default();
        let mut rx1 = bus.subscribe("test").unwrap();
        let mut rx2 = bus.subscribe("test").unwrap();

        bus.publish(BusMessage::new(
            "test",
            MessageType::Event,
            serde_json::json!({}),
        ))
        .unwrap();

        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }

    #[tokio::test]
    async fn topic_isolation() {
        let bus = InMemoryMessageBus::default();
        let mut rx_a = bus.subscribe("topic-a").unwrap();
        let mut rx_b = bus.subscribe("topic-b").unwrap();

        bus.publish(BusMessage::new(
            "topic-a",
            MessageType::Event,
            serde_json::json!({}),
        ))
        .unwrap();

        assert!(rx_a.recv().await.is_ok());
        assert!(rx_b.try_recv().is_err()); // not on topic-b
    }
}
