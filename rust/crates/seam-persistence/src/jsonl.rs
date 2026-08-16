use async_trait::async_trait;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use dsh_core_session::SessionEvent;

use super::trait_def::{SessionMetadata, SessionStore, StoreError};

/// JSONL-based session store.
///
/// Each session is stored as a separate `.jsonl` file in the base directory.
/// Session ID is the filename stem (without extension).
pub struct JsonlStore {
    base_dir: PathBuf,
}

impl JsonlStore {
    /// Create a new JSONL store backed by the given directory.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    fn session_path(&self, session_id: &str) -> PathBuf {
        self.base_dir.join(format!("{session_id}.jsonl"))
    }
}

#[async_trait]
impl SessionStore for JsonlStore {
    async fn create_session(&self) -> Result<String, StoreError> {
        let id = uuid::Uuid::new_v4().to_string();
        let path = self.session_path(&id);
        // Create the file to mark the session as existing
        tokio::fs::write(&path, "").await?;
        Ok(id)
    }

    async fn append(&self, session_id: &str, event: &SessionEvent) -> Result<(), StoreError> {
        let path = self.session_path(session_id);
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        let line = serde_json::to_string(event)?;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        Ok(())
    }

    async fn load(&self, session_id: &str) -> Result<Vec<SessionEvent>, StoreError> {
        let path = self.session_path(session_id);
        if !path.exists() {
            return Err(StoreError::NotFound(session_id.into()));
        }
        let file = tokio::fs::File::open(&path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut events = Vec::new();
        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }
            let event: SessionEvent = serde_json::from_str(&line)?;
            events.push(event);
        }
        Ok(events)
    }

    async fn metadata(&self, session_id: &str) -> Result<SessionMetadata, StoreError> {
        let events = self.load(session_id).await?;
        let path = self.session_path(session_id);
        let created_at = tokio::fs::metadata(&path)
            .await
            .and_then(|m| m.created())
            .map(|t| format!("{t:?}"))
            .unwrap_or_else(|_| "unknown".into());

        Ok(SessionMetadata {
            session_id: session_id.into(),
            event_count: events.len(),
            created_at,
        })
    }

    async fn list_sessions(&self) -> Result<Vec<SessionMetadata>, StoreError> {
        let mut sessions = Vec::new();
        let mut dir = tokio::fs::read_dir(&self.base_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(id) = name.strip_suffix(".jsonl")
                && let Ok(meta) = self.metadata(id).await
            {
                sessions.push(meta);
            }
        }
        Ok(sessions)
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), StoreError> {
        let path = self.session_path(session_id);
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn create_append_load() {
        let dir = tempdir().unwrap();
        let store = JsonlStore::new(dir.path());

        let id = store.create_session().await.unwrap();

        store
            .append(
                &id,
                &SessionEvent::UserMessage {
                    content: "hello".into(),
                },
            )
            .await
            .unwrap();
        store
            .append(
                &id,
                &SessionEvent::AssistantMessage {
                    content: "hi".into(),
                },
            )
            .await
            .unwrap();

        let events = store.load(&id).await.unwrap();
        assert_eq!(events.len(), 2);
    }

    #[tokio::test]
    async fn list_and_metadata() {
        let dir = tempdir().unwrap();
        let store = JsonlStore::new(dir.path());

        let id = store.create_session().await.unwrap();
        store
            .append(
                &id,
                &SessionEvent::UserMessage {
                    content: "test".into(),
                },
            )
            .await
            .unwrap();

        let sessions = store.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].event_count, 1);
    }

    #[tokio::test]
    async fn delete_session() {
        let dir = tempdir().unwrap();
        let store = JsonlStore::new(dir.path());

        let id = store.create_session().await.unwrap();
        store.delete_session(&id).await.unwrap();

        let result = store.load(&id).await;
        assert!(matches!(result, Err(StoreError::NotFound(_))));
    }
}
