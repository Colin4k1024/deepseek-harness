use async_trait::async_trait;
use dsh_core_session::SessionEvent;
use thiserror::Error;

/// Errors from session store operations.
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("database error: {0}")]
    Database(String),
    #[error("session not found: {0}")]
    NotFound(String),
}

/// Metadata about a stored session.
#[derive(Debug, Clone)]
pub struct SessionMetadata {
    pub session_id: String,
    pub event_count: usize,
    pub created_at: String,
}

/// Trait for durable session log storage.
///
/// Implementations must support:
/// - Append-only writes (events are never modified after storage)
/// - Sequential reads (load all events for a session)
/// - Session metadata queries
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Create a new session and return its ID.
    async fn create_session(&self) -> Result<String, StoreError>;

    /// Append an event to a session's log.
    async fn append(&self, session_id: &str, event: &SessionEvent) -> Result<(), StoreError>;

    /// Load all events for a session, in order.
    async fn load(&self, session_id: &str) -> Result<Vec<SessionEvent>, StoreError>;

    /// Get metadata about a session.
    async fn metadata(&self, session_id: &str) -> Result<SessionMetadata, StoreError>;

    /// List all sessions.
    async fn list_sessions(&self) -> Result<Vec<SessionMetadata>, StoreError>;

    /// Delete a session and all its events.
    async fn delete_session(&self, session_id: &str) -> Result<(), StoreError>;
}
