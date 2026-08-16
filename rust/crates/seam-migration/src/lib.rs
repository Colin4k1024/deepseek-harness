//! Database migration — schema versioning and migration management.
//!
//! Provides a migration system for managing database schema changes
//! with version tracking, rollback support, and migration history.

pub mod migration;
pub mod runner;
pub mod store;

pub use migration::{Migration, MigrationError, MigrationStep};
pub use runner::MigrationRunner;
pub use store::MigrationStore;
