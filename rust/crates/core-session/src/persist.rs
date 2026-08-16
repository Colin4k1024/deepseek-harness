use crate::event::SessionError;
use crate::event::SessionEvent;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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
            SessionEvent::UserMessage {
                content: "hello".into(),
            },
            SessionEvent::AssistantMessage {
                content: "hi".into(),
            },
        ];

        for e in &events {
            append_event(&path, e).await.unwrap();
        }

        let loaded = load_events(&path).await.unwrap();
        assert_eq!(loaded.len(), 2);

        // Cleanup
        let _ = tokio::fs::remove_file(&path).await;
    }
}
