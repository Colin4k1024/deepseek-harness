//! Message Bus — pub/sub message routing.

pub mod bus;
pub mod message;

pub use bus::{InMemoryMessageBus, MessageBus};
pub use message::{BusMessage, MessageType};
