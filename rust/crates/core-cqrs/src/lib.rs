//! CQRS — Command Query Responsibility Segregation.

pub mod command;
pub mod query;

pub use command::{Command, CommandHandler, CommandResult};
pub use query::{Query, QueryHandler, QueryResult};
