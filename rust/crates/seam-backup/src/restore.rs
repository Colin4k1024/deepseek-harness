//! Restore — restore session data from backups.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Restore errors.
#[derive(Debug, Error)]
pub enum RestoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("backup not found: {0}")]
    BackupNotFound(String),
    #[error("restore failed: {0}")]
    Failed(String),
}

/// Restore configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreConfig {
    /// Whether to overwrite existing sessions.
    pub overwrite: bool,

    /// Whether to validate data before restoring.
    pub validate: bool,
}

impl Default for RestoreConfig {
    fn default() -> Self {
        Self {
            overwrite: false,
            validate: true,
        }
    }
}

/// Result of a restore operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    /// Number of sessions restored.
    pub sessions_restored: usize,

    /// Number of events restored.
    pub events_restored: usize,

    /// Number of sessions skipped (already exist).
    pub sessions_skipped: usize,
}

/// Restore session data from a backup file.
pub async fn restore_from_file(
    path: &Path,
    config: &RestoreConfig,
) -> Result<RestoreResult, RestoreError> {
    let json = tokio::fs::read_to_string(path).await?;
    let data: super::backup::BackupData = serde_json::from_str(&json)?;

    if config.validate {
        validate_backup(&data)?;
    }

    let mut sessions_restored = 0;
    let mut events_restored = 0;

    for events in data.sessions.values() {
        // In a real implementation, this would write to the session store.
        sessions_restored += 1;
        events_restored += events.len();
    }

    tracing::info!(
        sessions = sessions_restored,
        events = events_restored,
        "restore completed"
    );

    Ok(RestoreResult {
        sessions_restored,
        events_restored,
        sessions_skipped: 0,
    })
}

/// Validate backup data integrity.
fn validate_backup(data: &super::backup::BackupData) -> Result<(), RestoreError> {
    if data.metadata.format_version.is_empty() {
        return Err(RestoreError::Failed("missing format version".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::backup::{BackupData, BackupMetadata};
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_restore_from_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test_backup.json");

        let data = BackupData {
            sessions: HashMap::new(),
            metadata: BackupMetadata {
                format_version: "1.0".into(),
                source: "test".into(),
                description: "test".into(),
            },
        };

        let json = serde_json::to_string_pretty(&data).unwrap();
        tokio::fs::write(&path, &json).await.unwrap();

        let config = RestoreConfig::default();
        let result = restore_from_file(&path, &config).await.unwrap();
        assert_eq!(result.sessions_restored, 0);
    }

    #[tokio::test]
    async fn validate_missing_version() {
        let data = BackupData {
            sessions: HashMap::new(),
            metadata: BackupMetadata {
                format_version: "".into(),
                source: "test".into(),
                description: "test".into(),
            },
        };

        let result = validate_backup(&data);
        assert!(result.is_err());
    }
}
