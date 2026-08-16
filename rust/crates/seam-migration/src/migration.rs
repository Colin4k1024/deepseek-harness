//! Migration definition and metadata.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Migration errors.
#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("migration not found: {0}")]
    NotFound(String),
    #[error("migration already applied: {0}")]
    AlreadyApplied(String),
    #[error("migration step failed: {0}")]
    StepFailed(String),
    #[error("rollback failed: {0}")]
    RollbackFailed(String),
    #[error("validation failed: {0}")]
    ValidationFailed(String),
}

/// A single migration step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStep {
    /// Step description.
    pub description: String,

    /// SQL to apply (or operation name for non-SQL stores).
    pub up: String,

    /// SQL to rollback (or operation name).
    pub down: String,
}

/// A migration with version, steps, and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    /// Unique version identifier (e.g., "001", "2026-08-15-add-users").
    pub version: String,

    /// Human-readable name.
    pub name: String,

    /// Migration steps (executed in order).
    pub steps: Vec<MigrationStep>,

    /// Whether this migration can be rolled back.
    pub rollback_supported: bool,

    /// Description of what this migration does.
    pub description: String,
}

impl Migration {
    /// Create a new migration.
    pub fn new(version: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            name: name.into(),
            steps: Vec::new(),
            rollback_supported: true,
            description: String::new(),
        }
    }

    /// Add a step to the migration.
    pub fn with_step(mut self, step: MigrationStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Set the description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Mark as non-rollbackable.
    pub fn no_rollback(mut self) -> Self {
        self.rollback_supported = false;
        self
    }
}

/// Record of an applied migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRecord {
    /// Migration version.
    pub version: String,

    /// When it was applied (ISO 8601).
    pub applied_at: String,

    /// Duration in milliseconds.
    pub duration_ms: u64,

    /// Whether it was successful.
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_migration() {
        let m = Migration::new("001", "create_users");
        assert_eq!(m.version, "001");
        assert_eq!(m.name, "create_users");
        assert!(m.steps.is_empty());
        assert!(m.rollback_supported);
    }

    #[test]
    fn migration_with_steps() {
        let m = Migration::new("001", "create_users")
            .with_description("Create the users table")
            .with_step(MigrationStep {
                description: "Create users table".into(),
                up: "CREATE TABLE users (id INTEGER PRIMARY KEY)".into(),
                down: "DROP TABLE users".into(),
            });

        assert_eq!(m.steps.len(), 1);
        assert_eq!(m.description, "Create the users table");
    }

    #[test]
    fn no_rollback() {
        let m = Migration::new("002", "data_migration").no_rollback();
        assert!(!m.rollback_supported);
    }
}
