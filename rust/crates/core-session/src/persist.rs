use crate::event::SessionError;
use crate::event::SessionEvent;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Session metadata header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHeader {
    pub id: String,
    pub format_version: u32,
    pub created_at: u64,
    pub updated_at: u64,
    pub title: String,
    pub event_count: u64,
    pub last_seq: u64,
    pub is_seed: bool,
}

impl SessionHeader {
    pub fn new(id: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            id: id.into(),
            format_version: 0,
            created_at: now,
            updated_at: now,
            title: String::new(),
            event_count: 0,
            last_seq: 0,
            is_seed: true,
        }
    }
}

/// Append a session event to a JSONL file.
pub async fn append_event(path: &Path, event: &SessionEvent) -> Result<(), SessionError> {
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    let line = serde_json::to_string(event)?;
    file.write_all(line.as_bytes()).await?;
    file.write_all(b"\n").await?;
    Ok(())
}

/// Load all session events from a JSONL file.
pub async fn load_events(path: &Path) -> Result<Vec<SessionEvent>, SessionError> {
    let file = tokio::fs::File::open(path).await?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut events = Vec::new();
    while let Some(line) = lines.next_line().await? {
        let event: SessionEvent = serde_json::from_str(&line)?;
        events.push(event);
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn roundtrip_jsonl() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().with_extension("jsonl");

        let events = vec![
            SessionEvent::TurnStart { seq: 1, time: 1000, turn: 1 },
            SessionEvent::UserMessage {
                seq: 2, time: 1001, content: "hello".into(), turn: Some(1), source: None,
            },
            SessionEvent::AssistantMessage {
                seq: 3, time: 1002, turn: 1, step: 1, message: "hi".into(), usage: None,
                source_event_seqs: None,
            },
            SessionEvent::TurnEnd {
                seq: 4, time: 1003, turn: 1, reason: crate::event::TurnEndReason::Completed,
            },
        ];

        for e in &events {
            append_event(&path, e).await.unwrap();
        }

        let loaded = load_events(&path).await.unwrap();
        assert_eq!(loaded.len(), 4);

        // Cleanup
        let _ = tokio::fs::remove_file(&path).await;
    }
}