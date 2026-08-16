//! Event Store — append-only event log with replay capability.

pub mod event;
pub mod store;

pub use event::{DomainEvent, EventEnvelope, EventId};
pub use store::{EventStore, InMemoryEventStore};
