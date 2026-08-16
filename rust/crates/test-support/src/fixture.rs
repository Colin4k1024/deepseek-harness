use std::path::Path;

use dsh_core_session::event::SessionEvent;

/// Load session events from a JSONL fixture file.
pub struct FixtureLoader;

impl FixtureLoader {
    /// Load events from a JSONL file.
    pub async fn load(path: &Path) -> Result<Vec<SessionEvent>, serde_json::Error> {
        let content = tokio::fs::read_to_string(path).await.unwrap_or_default();
        let mut events = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let event: SessionEvent = serde_json::from_str(line)?;
            events.push(event);
        }
        Ok(events)
    }
}
