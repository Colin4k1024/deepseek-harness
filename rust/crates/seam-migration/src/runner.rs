//! Migration runner — executes migrations in order.

use std::sync::Arc;
use std::time::Instant;

use super::migration::{Migration, MigrationError, MigrationRecord};
use super::store::MigrationStore;

/// Migration runner — executes and tracks migrations.
pub struct MigrationRunner {
    store: Arc<dyn MigrationStore>,
}

impl MigrationRunner {
    /// Create a new migration runner.
    pub fn new(store: Arc<dyn MigrationStore>) -> Self {
        Self { store }
    }

    /// Run pending migrations from a list.
    ///
    /// Only runs migrations that haven't been applied yet, in order.
    pub async fn run(
        &self,
        migrations: &[Migration],
    ) -> Result<Vec<MigrationRecord>, MigrationError> {
        let mut records = Vec::new();

        for migration in migrations {
            if self.store.is_applied(&migration.version).await? {
                tracing::info!(version = %migration.version, "migration already applied, skipping");
                continue;
            }

            let record = self.run_migration(migration).await?;
            self.store.record(record.clone()).await?;
            records.push(record);
        }

        Ok(records)
    }

    /// Run a single migration.
    async fn run_migration(
        &self,
        migration: &Migration,
    ) -> Result<MigrationRecord, MigrationError> {
        tracing::info!(version = %migration.version, name = %migration.name, "running migration");
        let start = Instant::now();

        for (i, step) in migration.steps.iter().enumerate() {
            tracing::debug!(step = i + 1, description = %step.description, "executing step");
            // In a real implementation, this would execute the SQL or operation.
            // For now, we just validate the step is non-empty.
            if step.up.is_empty() {
                return Err(MigrationError::StepFailed(format!(
                    "step {} of migration {} has empty up",
                    i + 1,
                    migration.version
                )));
            }
        }

        let duration = start.elapsed();
        tracing::info!(
            version = %migration.version,
            duration_ms = duration.as_millis(),
            "migration completed"
        );

        Ok(MigrationRecord {
            version: migration.version.clone(),
            applied_at: chrono_now(),
            duration_ms: duration.as_millis() as u64,
            success: true,
        })
    }

    /// Rollback the last applied migration.
    pub async fn rollback_last(&self, migrations: &[Migration]) -> Result<(), MigrationError> {
        let applied = self.store.applied().await?;
        let last = applied
            .last()
            .ok_or_else(|| MigrationError::NotFound("no migrations to rollback".into()))?;

        let migration = migrations
            .iter()
            .find(|m| m.version == last.version)
            .ok_or_else(|| MigrationError::NotFound(last.version.clone()))?;

        if !migration.rollback_supported {
            return Err(MigrationError::RollbackFailed(format!(
                "migration {} does not support rollback",
                migration.version
            )));
        }

        tracing::info!(version = %migration.version, "rolling back migration");
        self.store.remove(&migration.version).await?;
        tracing::info!(version = %migration.version, "rollback completed");

        Ok(())
    }

    /// Get migration status.
    pub async fn status(
        &self,
        migrations: &[Migration],
    ) -> Result<Vec<MigrationStatus>, MigrationError> {
        let mut statuses = Vec::new();
        for migration in migrations {
            let applied = self.store.is_applied(&migration.version).await?;
            statuses.push(MigrationStatus {
                version: migration.version.clone(),
                name: migration.name.clone(),
                applied,
                rollback_supported: migration.rollback_supported,
            });
        }
        Ok(statuses)
    }
}

/// Status of a migration.
#[derive(Debug, Clone)]
pub struct MigrationStatus {
    pub version: String,
    pub name: String,
    pub applied: bool,
    pub rollback_supported: bool,
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::super::migration::MigrationStep;
    use super::super::store::InMemoryMigrationStore;
    use super::*;

    fn test_migrations() -> Vec<Migration> {
        vec![
            Migration::new("001", "create_users").with_step(MigrationStep {
                description: "Create users table".into(),
                up: "CREATE TABLE users (id INTEGER PRIMARY KEY)".into(),
                down: "DROP TABLE users".into(),
            }),
            Migration::new("002", "add_email").with_step(MigrationStep {
                description: "Add email column".into(),
                up: "ALTER TABLE users ADD COLUMN email TEXT".into(),
                down: "ALTER TABLE users DROP COLUMN email".into(),
            }),
        ]
    }

    #[tokio::test]
    async fn run_pending_migrations() {
        let store = Arc::new(InMemoryMigrationStore::new());
        let runner = MigrationRunner::new(store.clone());

        let records = runner.run(&test_migrations()).await.unwrap();
        assert_eq!(records.len(), 2);
        assert!(records[0].success);
        assert!(records[1].success);
    }

    #[tokio::test]
    async fn skip_already_applied() {
        let store = Arc::new(InMemoryMigrationStore::new());
        let runner = MigrationRunner::new(store.clone());

        runner.run(&test_migrations()).await.unwrap();
        let records = runner.run(&test_migrations()).await.unwrap();
        assert!(records.is_empty()); // All already applied
    }

    #[tokio::test]
    async fn rollback_last() {
        let store = Arc::new(InMemoryMigrationStore::new());
        let runner = MigrationRunner::new(store.clone());

        runner.run(&test_migrations()).await.unwrap();
        assert!(store.is_applied("002").await.unwrap());

        runner.rollback_last(&test_migrations()).await.unwrap();
        assert!(!store.is_applied("002").await.unwrap());
        assert!(store.is_applied("001").await.unwrap());
    }

    #[tokio::test]
    async fn status_report() {
        let store = Arc::new(InMemoryMigrationStore::new());
        let runner = MigrationRunner::new(store.clone());

        runner.run(&[test_migrations()[0].clone()]).await.unwrap();

        let status = runner.status(&test_migrations()).await.unwrap();
        assert!(status[0].applied);
        assert!(!status[1].applied);
    }
}
