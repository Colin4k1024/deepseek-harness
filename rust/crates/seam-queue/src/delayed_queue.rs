//! Delayed queue — messages are delivered after their delay expires.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

use super::message::QueueMessage;

/// Queue configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    /// Maximum queue size.
    pub max_size: usize,

    /// Default delay in seconds.
    pub default_delay_secs: u64,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_size: 10000,
            default_delay_secs: 0,
        }
    }
}

/// Entry in the delayed queue with delivery time.
struct QueueEntry {
    message: QueueMessage,
    deliver_at: Instant,
}

/// Delayed queue — delivers messages after their delay expires.
pub struct DelayedQueue {
    config: QueueConfig,
    queue: Mutex<VecDeque<QueueEntry>>,
}

impl DelayedQueue {
    /// Create a new delayed queue.
    pub fn new(config: QueueConfig) -> Self {
        Self {
            config,
            queue: Mutex::new(VecDeque::new()),
        }
    }

    /// Enqueue a message.
    pub fn enqueue(&self, message: QueueMessage) -> Result<(), QueueError> {
        let mut queue = self.queue.lock().unwrap();
        if queue.len() >= self.config.max_size {
            return Err(QueueError::QueueFull);
        }

        let delay = if message.delay_secs > 0 {
            message.delay_secs
        } else {
            self.config.default_delay_secs
        };

        queue.push_back(QueueEntry {
            deliver_at: Instant::now() + std::time::Duration::from_secs(delay),
            message,
        });

        Ok(())
    }

    /// Dequeue the next ready message.
    pub fn dequeue(&self) -> Option<QueueMessage> {
        let mut queue = self.queue.lock().unwrap();
        let now = Instant::now();

        // Find the first entry whose delay has expired
        for i in 0..queue.len() {
            if queue[i].deliver_at <= now {
                return Some(queue.remove(i).unwrap().message);
            }
        }

        None
    }

    /// Peek at the next message without removing it.
    pub fn peek(&self) -> Option<QueueMessage> {
        let queue = self.queue.lock().unwrap();
        let now = Instant::now();

        queue
            .iter()
            .find(|e| e.deliver_at <= now)
            .map(|e| e.message.clone())
    }

    /// Queue length.
    pub fn len(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.lock().unwrap().is_empty()
    }

    /// Remove a message by ID.
    pub fn remove(&self, message_id: &str) -> bool {
        let mut queue = self.queue.lock().unwrap();
        if let Some(pos) = queue.iter().position(|e| e.message.id == message_id) {
            queue.remove(pos);
            true
        } else {
            false
        }
    }
}

impl Default for DelayedQueue {
    fn default() -> Self {
        Self::new(QueueConfig::default())
    }
}

/// Queue errors.
#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("queue is full")]
    QueueFull,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immediate_delivery() {
        let queue = DelayedQueue::default();
        queue
            .enqueue(QueueMessage::new("test", serde_json::json!({"a": 1})))
            .unwrap();

        let msg = queue.dequeue().unwrap();
        assert_eq!(msg.queue, "test");
        assert!(queue.is_empty());
    }

    #[test]
    fn delayed_delivery() {
        let queue = DelayedQueue::default();
        queue
            .enqueue(
                QueueMessage::new("test", serde_json::json!({})).with_delay(3600), // 1 hour
            )
            .unwrap();

        assert!(queue.dequeue().is_none()); // not ready yet
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn queue_full() {
        let queue = DelayedQueue::new(QueueConfig {
            max_size: 2,
            ..Default::default()
        });

        queue
            .enqueue(QueueMessage::new("q", serde_json::json!({})))
            .unwrap();
        queue
            .enqueue(QueueMessage::new("q", serde_json::json!({})))
            .unwrap();
        assert!(
            queue
                .enqueue(QueueMessage::new("q", serde_json::json!({})))
                .is_err()
        );
    }

    #[test]
    fn remove_message() {
        let queue = DelayedQueue::default();
        let msg = QueueMessage::new("q", serde_json::json!({}));
        let id = msg.id.clone();
        queue.enqueue(msg).unwrap();

        assert!(queue.remove(&id));
        assert!(queue.is_empty());
    }

    #[test]
    fn peek_message() {
        let queue = DelayedQueue::default();
        queue
            .enqueue(QueueMessage::new("q", serde_json::json!({})))
            .unwrap();

        let msg = queue.peek().unwrap();
        assert_eq!(msg.queue, "q");
        assert_eq!(queue.len(), 1); // still in queue
    }
}
