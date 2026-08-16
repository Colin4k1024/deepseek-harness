use async_trait::async_trait;
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

use dsh_core_session::SessionEvent;

use super::trait_def::{SessionMetadata, SessionStore, StoreError};

/// SQLite-based session store.
///
/// All sessions are stored in a single SQLite database with two tables:
/// - `sessions`: session metadata (id, created_at)
/// - `events`: session events (session_id, seq, data)
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    /// Create or open a SQLite store at the given path.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path).map_err(|e| StoreError::Database(e.to_string()))?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS events (
                session_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                data TEXT NOT NULL,
                PRIMARY KEY (session_id, seq),
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );
            ",
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create an in-memory SQLite store (for testing).
    pub fn memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory().map_err(|e| StoreError::Database(e.to_string()))?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS events (
                session_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                data TEXT NOT NULL,
                PRIMARY KEY (session_id, seq),
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );
            ",
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

#[async_trait]
impl SessionStore for SqliteStore {
    async fn create_session(&self) -> Result<String, StoreError> {
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn.lock().unwrap();
        conn.execute("INSERT INTO sessions (id) VALUES (?1)", params![id])
            .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(id)
    }

    async fn append(&self, session_id: &str, event: &SessionEvent) -> Result<(), StoreError> {
        let data = serde_json::to_string(event)?;
        let conn = self.conn.lock().unwrap();

        // Get the next sequence number
        let seq: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(seq), -1) + 1 FROM events WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .map_err(|e| StoreError::Database(e.to_string()))?;

        conn.execute(
            "INSERT INTO events (session_id, seq, data) VALUES (?1, ?2, ?3)",
            params![session_id, seq, data],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;

        Ok(())
    }

    async fn load(&self, session_id: &str) -> Result<Vec<SessionEvent>, StoreError> {
        let conn = self.conn.lock().unwrap();

        // Check session exists
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE id = ?1",
                params![session_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .map_err(|e| StoreError::Database(e.to_string()))?;

        if !exists {
            return Err(StoreError::NotFound(session_id.into()));
        }

        let mut stmt = conn
            .prepare("SELECT data FROM events WHERE session_id = ?1 ORDER BY seq")
            .map_err(|e| StoreError::Database(e.to_string()))?;

        let events = stmt
            .query_map(params![session_id], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })
            .map_err(|e| StoreError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| StoreError::Database(e.to_string()))?;

        events
            .iter()
            .map(|data| serde_json::from_str(data).map_err(StoreError::Serialization))
            .collect()
    }

    async fn metadata(&self, session_id: &str) -> Result<SessionMetadata, StoreError> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT s.created_at, COUNT(e.seq) FROM sessions s LEFT JOIN events e ON s.id = e.session_id WHERE s.id = ?1 GROUP BY s.id",
            params![session_id],
            |row| {
                let created_at: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok((created_at, count as usize))
            },
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound(session_id.into()),
            other => StoreError::Database(other.to_string()),
        })?;

        Ok(SessionMetadata {
            session_id: session_id.into(),
            event_count: result.1,
            created_at: result.0,
        })
    }

    async fn list_sessions(&self) -> Result<Vec<SessionMetadata>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT s.id, s.created_at, COUNT(e.seq) FROM sessions s LEFT JOIN events e ON s.id = e.session_id GROUP BY s.id",
            )
            .map_err(|e| StoreError::Database(e.to_string()))?;

        let sessions = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let created_at: String = row.get(1)?;
                let count: i64 = row.get(2)?;
                Ok(SessionMetadata {
                    session_id: id,
                    event_count: count as usize,
                    created_at,
                })
            })
            .map_err(|e| StoreError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| StoreError::Database(e.to_string()))?;

        Ok(sessions)
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM events WHERE session_id = ?1",
            params![session_id],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![session_id])
            .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_append_load() {
        let store = SqliteStore::memory().unwrap();
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
        let store = SqliteStore::memory().unwrap();
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
        let store = SqliteStore::memory().unwrap();
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

        store.delete_session(&id).await.unwrap();

        let result = store.load(&id).await;
        assert!(matches!(result, Err(StoreError::NotFound(_))));
    }

    #[tokio::test]
    async fn event_ordering() {
        let store = SqliteStore::memory().unwrap();
        let id = store.create_session().await.unwrap();

        for i in 0..10 {
            store
                .append(
                    &id,
                    &SessionEvent::UserMessage {
                        content: format!("msg {i}"),
                    },
                )
                .await
                .unwrap();
        }

        let events = store.load(&id).await.unwrap();
        assert_eq!(events.len(), 10);
        // Verify ordering
        for (i, event) in events.iter().enumerate() {
            if let SessionEvent::UserMessage { content } = event {
                assert_eq!(content, &format!("msg {i}"));
            }
        }
    }
}
