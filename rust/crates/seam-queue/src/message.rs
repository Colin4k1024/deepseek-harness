//! Queue message definition.

use serde::{Deserialize, Serialize};

/// Unique message identifier.
pub type MessageId = String;

/// A message in the delayed queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueMessage {
    /// Message ID.
    pub id: MessageId,

    /// Queue/topic name.
    pub queue: String,

    /// Message payload.
    pub payload: serde_json::Value,

    /// Delay in seconds before delivery.
    pub delay_secs: u64,

    /// Maximum delivery attempts.
    pub max_attempts: u32,

    /// Current attempt count.
    pub attempts: u32,
}

impl QueueMessage {
    /// Create a new message.
    pub fn new(queue: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            queue: queue.into(),
            payload,
            delay_secs: 0,
            max_attempts: 3,
            attempts: 0,
        }
    }

    /// Set the delay.
    pub fn with_delay(mut self, delay_secs: u64) -> Self {
        self.delay_secs = delay_secs;
        self
    }

    /// Set max attempts.
    pub fn with_max_attempts(mut self, max: u32) -> Self {
        self.max_attempts = max;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_message() {
        let msg = QueueMessage::new("tasks", serde_json::json!({"action": "process"}));
        assert_eq!(msg.queue, "tasks");
        assert_eq!(msg.delay_secs, 0);
        assert_eq!(msg.max_attempts, 3);
    }

    #[test]
    fn message_builder() {
        let msg = QueueMessage::new("tasks", serde_json::json!({}))
            .with_delay(60)
            .with_max_attempts(5);

        assert_eq!(msg.delay_secs, 60);
        assert_eq!(msg.max_attempts, 5);
    }
}
