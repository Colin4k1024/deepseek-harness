//! Bus message types.

use serde::{Deserialize, Serialize};

/// Message type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    Event,
    Command,
    Query,
    Notification,
}

/// A message on the bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusMessage {
    /// Message ID.
    pub id: String,

    /// Topic/channel.
    pub topic: String,

    /// Message type.
    pub message_type: MessageType,

    /// Payload.
    pub payload: serde_json::Value,

    /// Timestamp.
    pub timestamp: u64,
}

impl BusMessage {
    /// Create a new message.
    pub fn new(
        topic: impl Into<String>,
        message_type: MessageType,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            topic: topic.into(),
            message_type,
            payload,
            timestamp: now(),
        }
    }
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
    fn new_message() {
        let msg = BusMessage::new("user.created", MessageType::Event, serde_json::json!({}));
        assert_eq!(msg.topic, "user.created");
        assert_eq!(msg.message_type, MessageType::Event);
    }
}
