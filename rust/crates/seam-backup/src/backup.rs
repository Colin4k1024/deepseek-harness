//! Backup — create snapshots of session data.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Backup errors.
#[derive(Debug, Error)]
pub enum BackupError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("backup not found: {0}")]
    NotFound(String),
    #[error("backup failed: {0}")]
    Failed(String),
}

/// Backup configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    /// Backup directory.
    pub backup_dir: PathBuf,

    /// Maximum number of backups to keep.
    pub max_backups: usize,

    /// Whether to compress backups.
    pub compress: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            backup_dir: PathBuf::from("./backups"),
            max_backups: 10,
            compress: false,
        }
    }
}

/// Result of a backup operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    /// Backup ID (timestamp-based).
    pub backup_id: String,

    /// Path to the backup file.
    pub path: PathBuf,

    /// Number of sessions backed up.
    pub session_count: usize,

    /// Total events backed up.
    pub event_count: usize,

    /// Backup size in bytes.
    pub size_bytes: u64,

    /// Creation timestamp.
    pub created_at: String,
}

/// Trait for backup storage backends.
#[async_trait]
pub trait BackupStore: Send + Sync {
    /// Create a backup.
    async fn create_backup(&self, data: &BackupData) -> Result<BackupResult, BackupError>;

    /// List available backups.
    async fn list_backups(&self) -> Result<Vec<BackupResult>, BackupError>;

    /// Delete a backup.
    async fn delete_backup(&self, backup_id: &str) -> Result<(), BackupError>;
}

/// Data to be backed up.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupData {
    /// Session events (session_id → events).
    pub sessions: HashMap<String, Vec<serde_json::Value>>,

    /// Metadata.
    pub metadata: BackupMetadata,
}

use std::collections::HashMap;

/// Backup metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    /// Version of the backup format.
    pub format_version: String,

    /// Source identifier.
    pub source: String,

    /// Description.
    pub description: String,
}

/// File-based backup store.
pub struct FileBackupStore {
    config: BackupConfig,
}

impl FileBackupStore {
    pub fn new(config: BackupConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl BackupStore for FileBackupStore {
    async fn create_backup(&self, data: &BackupData) -> Result<BackupResult, BackupError> {
        tokio::fs::create_dir_all(&self.config.backup_dir).await?;

        let backup_id = format!(
            "backup_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );

        let path = self.config.backup_dir.join(format!("{backup_id}.json"));
        let json = serde_json::to_string_pretty(data)?;
        tokio::fs::write(&path, &json).await?;

        let session_count = data.sessions.len();
        let event_count = data.sessions.values().map(|e| e.len()).sum();

        tracing::info!(
            backup_id = %backup_id,
            sessions = session_count,
            events = event_count,
            "backup created"
        );

        Ok(BackupResult {
            backup_id,
            path,
            session_count,
            event_count,
            size_bytes: json.len() as u64,
            created_at: chrono_now(),
        })
    }

    async fn list_backups(&self) -> Result<Vec<BackupResult>, BackupError> {
        let mut backups = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.config.backup_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".json") {
                let path = entry.path();
                let metadata = tokio::fs::metadata(&path).await?;
                let backup_id = name.replace(".json", "");
                backups.push(BackupResult {
                    backup_id,
                    path,
                    session_count: 0, // Would need to read file to get this
                    event_count: 0,
                    size_bytes: metadata.len(),
                    created_at: chrono_now(),
                });
            }
        }

        Ok(backups)
    }

    async fn delete_backup(&self, backup_id: &str) -> Result<(), BackupError> {
        let path = self.config.backup_dir.join(format!("{backup_id}.json"));
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }
        Ok(())
    }
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
    use super::*;

    #[tokio::test]
    async fn create_and_list_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let config = BackupConfig {
            backup_dir: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let store = FileBackupStore::new(config);

        let data = BackupData {
            sessions: HashMap::new(),
            metadata: BackupMetadata {
                format_version: "1.0".into(),
                source: "test".into(),
                description: "test backup".into(),
            },
        };

        let result = store.create_backup(&data).await.unwrap();
        assert!(!result.backup_id.is_empty());

        let backups = store.list_backups().await.unwrap();
        assert_eq!(backups.len(), 1);
    }

    #[tokio::test]
    async fn delete_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let config = BackupConfig {
            backup_dir: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let store = FileBackupStore::new(config);

        let data = BackupData {
            sessions: HashMap::new(),
            metadata: BackupMetadata {
                format_version: "1.0".into(),
                source: "test".into(),
                description: "test".into(),
            },
        };

        let result = store.create_backup(&data).await.unwrap();
        store.delete_backup(&result.backup_id).await.unwrap();

        let backups = store.list_backups().await.unwrap();
        assert!(backups.is_empty());
    }
}
