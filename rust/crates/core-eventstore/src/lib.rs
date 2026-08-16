//! Event store — append-only event log, CQRS, and message bus.

pub mod bus;
pub mod bus_message;
pub mod command;
pub mod event;
pub mod query;
pub mod store;

pub use bus::InMemoryMessageBus;
pub use bus_message::BusMessage;
pub use command::{Command, CommandHandler, SimpleCommandHandler};
pub use event::EventEnvelope;
pub use query::{Query, QueryHandler};
pub use store::{EventStore, InMemoryEventStore};
