//! Migration store — tracks applied migrations.

use async_trait::async_trait;

use super::migration::{MigrationError, MigrationRecord};

/// Trait for storing migration history.
#[async_trait]
pub trait MigrationStore: Send + Sync {
    /// Get all applied migration records.
    async fn applied(&self) -> Result<Vec<MigrationRecord>, MigrationError>;

    /// Record a migration as applied.
    async fn record(&self, record: MigrationRecord) -> Result<(), MigrationError>;

    /// Remove a migration record (for rollback).
    async fn remove(&self, version: &str) -> Result<(), MigrationError>;

    /// Check if a version has been applied.
    async fn is_applied(&self, version: &str) -> Result<bool, MigrationError>;
}

/// In-memory migration store (for testing).
pub struct InMemoryMigrationStore {
    records: tokio::sync::RwLock<Vec<MigrationRecord>>,
}

impl InMemoryMigrationStore {
    pub fn new() -> Self {
        Self {
            records: tokio::sync::RwLock::new(Vec::new()),
        }
    }
}

impl Default for InMemoryMigrationStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MigrationStore for InMemoryMigrationStore {
    async fn applied(&self) -> Result<Vec<MigrationRecord>, MigrationError> {
        let records = self.records.read().await;
        Ok(records.clone())
    }

    async fn record(&self, record: MigrationRecord) -> Result<(), MigrationError> {
        let mut records = self.records.write().await;
        records.push(record);
        Ok(())
    }

    async fn remove(&self, version: &str) -> Result<(), MigrationError> {
        let mut records = self.records.write().await;
        records.retain(|r| r.version != version);
        Ok(())
    }

    async fn is_applied(&self, version: &str) -> Result<bool, MigrationError> {
        let records = self.records.read().await;
        Ok(records.iter().any(|r| r.version == version && r.success))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn record_and_query() {
        let store = InMemoryMigrationStore::new();
        assert!(!store.is_applied("001").await.unwrap());

        store
            .record(MigrationRecord {
                version: "001".into(),
                applied_at: "2026-08-15T00:00:00Z".into(),
                duration_ms: 100,
                success: true,
            })
            .await
            .unwrap();

        assert!(store.is_applied("001").await.unwrap());
        assert_eq!(store.applied().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn remove_migration() {
        let store = InMemoryMigrationStore::new();
        store
            .record(MigrationRecord {
                version: "001".into(),
                applied_at: "2026-08-15T00:00:00Z".into(),
                duration_ms: 100,
                success: true,
            })
            .await
            .unwrap();

        store.remove("001").await.unwrap();
        assert!(!store.is_applied("001").await.unwrap());
    }
}
