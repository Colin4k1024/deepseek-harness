//! Delayed queue — delayed message delivery with timing guarantees.

pub mod delayed_queue;
pub mod message;

pub use delayed_queue::{DelayedQueue, QueueConfig};
pub use message::{MessageId, QueueMessage};
