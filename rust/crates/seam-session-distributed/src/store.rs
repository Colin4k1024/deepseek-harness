//! DistributedSessionStore — trait and configuration for distributed session storage.
//!
//! Extends the local SessionStore with distributed features:
//! - Session locking (prevent concurrent writes from multiple nodes)
//! - TTL-based expiration
//! - Cross-node session discovery

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use dsh_core_session::SessionEvent;
use dsh_seam_persistence::{SessionMetadata, SessionStore, StoreError};

/// Configuration for a distributed session store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedSessionConfig {
    /// Connection URL (e.g., "redis://localhost:6379", "postgres://...").
    pub url: String,

    /// Session TTL in seconds (0 = no expiration).
    pub ttl_secs: u64,

    /// Lock timeout in seconds.
    pub lock_timeout_secs: u64,

    /// Node ID (unique per instance).
    pub node_id: String,
}

impl Default for DistributedSessionConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".into(),
            ttl_secs: 3600,        // 1 hour
            lock_timeout_secs: 30, // 30 seconds
            node_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}

/// A session lock — prevents concurrent writes from multiple nodes.
#[derive(Debug, Clone)]
pub struct SessionLock {
    /// Lock key.
    pub key: String,

    /// Node that holds the lock.
    pub node_id: String,

    /// When the lock was acquired.
    pub acquired_at: Instant,

    /// Lock TTL.
    pub ttl: Duration,
}

/// Trait for distributed session stores.
///
/// Extends `SessionStore` with distributed features.
#[async_trait]
pub trait DistributedSessionStore: SessionStore + Send + Sync {
    /// Acquire a lock on a session (for exclusive write access).
    async fn acquire_lock(
        &self,
        session_id: &str,
        node_id: &str,
        timeout: Duration,
    ) -> Result<SessionLock, StoreError>;

    /// Release a session lock.
    async fn release_lock(&self, lock: &SessionLock) -> Result<(), StoreError>;

    /// Extend a session's TTL.
    async fn touch(&self, session_id: &str) -> Result<(), StoreError>;

    /// Get all sessions on a specific node.
    async fn sessions_on_node(&self, node_id: &str) -> Result<Vec<SessionMetadata>, StoreError>;

    /// Get store health/status.
    async fn health(&self) -> Result<HealthStatus, StoreError>;
}

/// Health status of a distributed session store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub connected: bool,
    pub node_count: usize,
    pub total_sessions: usize,
    pub latency_ms: u64,
}

/// In-memory implementation of DistributedSessionStore (for testing/development).
///
/// Simulates distributed behavior with local state.
pub struct InMemoryDistributedStore {
    sessions: RwLock<HashMap<String, Vec<SessionEvent>>>,
    locks: RwLock<HashMap<String, SessionLock>>,
    #[allow(dead_code)] // Will be used when Redis/Postgres backends are implemented
    config: DistributedSessionConfig,
}

impl InMemoryDistributedStore {
    pub fn new(config: DistributedSessionConfig) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            locks: RwLock::new(HashMap::new()),
            config,
        }
    }
}

#[async_trait]
impl SessionStore for InMemoryDistributedStore {
    async fn create_session(&self) -> Result<String, StoreError> {
        let id = uuid::Uuid::new_v4().to_string();
        self.sessions
            .write()
            .unwrap()
            .insert(id.clone(), Vec::new());
        Ok(id)
    }

    async fn append(&self, session_id: &str, event: &SessionEvent) -> Result<(), StoreError> {
        let mut sessions = self.sessions.write().unwrap();
        if let Some(events) = sessions.get_mut(session_id) {
            events.push(event.clone());
            Ok(())
        } else {
            Err(StoreError::NotFound(session_id.into()))
        }
    }

    async fn load(&self, session_id: &str) -> Result<Vec<SessionEvent>, StoreError> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(session_id.into()))
    }

    async fn metadata(&self, session_id: &str) -> Result<SessionMetadata, StoreError> {
        let sessions = self.sessions.read().unwrap();
        if let Some(events) = sessions.get(session_id) {
            Ok(SessionMetadata {
                session_id: session_id.into(),
                event_count: events.len(),
                created_at: "now".into(),
            })
        } else {
            Err(StoreError::NotFound(session_id.into()))
        }
    }

    async fn list_sessions(&self) -> Result<Vec<SessionMetadata>, StoreError> {
        let sessions = self.sessions.read().unwrap();
        Ok(sessions
            .iter()
            .map(|(id, events)| SessionMetadata {
                session_id: id.clone(),
                event_count: events.len(),
                created_at: "now".into(),
            })
            .collect())
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), StoreError> {
        self.sessions.write().unwrap().remove(session_id);
        Ok(())
    }
}

#[async_trait]
impl DistributedSessionStore for InMemoryDistributedStore {
    async fn acquire_lock(
        &self,
        session_id: &str,
        node_id: &str,
        timeout: Duration,
    ) -> Result<SessionLock, StoreError> {
        let mut locks = self.locks.write().unwrap();

        // Check if already locked
        if let Some(existing) = locks.get(session_id)
            && existing.acquired_at.elapsed() < existing.ttl
        {
            return Err(StoreError::Database(format!(
                "session {} is locked by node {}",
                session_id, existing.node_id
            )));
        }

        let lock = SessionLock {
            key: session_id.into(),
            node_id: node_id.into(),
            acquired_at: Instant::now(),
            ttl: timeout,
        };
        locks.insert(session_id.into(), lock.clone());
        Ok(lock)
    }

    async fn release_lock(&self, lock: &SessionLock) -> Result<(), StoreError> {
        self.locks.write().unwrap().remove(&lock.key);
        Ok(())
    }

    async fn touch(&self, _session_id: &str) -> Result<(), StoreError> {
        // In-memory doesn't have real TTL
        Ok(())
    }

    async fn sessions_on_node(&self, _node_id: &str) -> Result<Vec<SessionMetadata>, StoreError> {
        // In-memory doesn't track node ownership
        self.list_sessions().await
    }

    async fn health(&self) -> Result<HealthStatus, StoreError> {
        let sessions = self.sessions.read().unwrap();
        Ok(HealthStatus {
            connected: true,
            node_count: 1,
            total_sessions: sessions.len(),
            latency_ms: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsh_core_session::SessionEvent;

    fn test_config() -> DistributedSessionConfig {
        DistributedSessionConfig {
            url: "memory://localhost".into(),
            ttl_secs: 3600,
            lock_timeout_secs: 30,
            node_id: "test-node-1".into(),
        }
    }

    #[tokio::test]
    async fn create_and_append() {
        let store = InMemoryDistributedStore::new(test_config());
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

        let events = store.load(&id).await.unwrap();
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn lock_and_release() {
        let store = InMemoryDistributedStore::new(test_config());
        let id = store.create_session().await.unwrap();

        let lock = store
            .acquire_lock(&id, "node-1", Duration::from_secs(30))
            .await
            .unwrap();
        assert_eq!(lock.node_id, "node-1");

        // Second lock attempt should fail
        let result = store
            .acquire_lock(&id, "node-2", Duration::from_secs(30))
            .await;
        assert!(result.is_err());

        // Release and re-acquire
        store.release_lock(&lock).await.unwrap();
        let lock2 = store
            .acquire_lock(&id, "node-2", Duration::from_secs(30))
            .await
            .unwrap();
        assert_eq!(lock2.node_id, "node-2");
    }

    #[tokio::test]
    async fn health_check() {
        let store = InMemoryDistributedStore::new(test_config());
        let health = store.health().await.unwrap();
        assert!(health.connected);
        assert_eq!(health.node_count, 1);
    }

    #[tokio::test]
    async fn list_sessions() {
        let store = InMemoryDistributedStore::new(test_config());
        store.create_session().await.unwrap();
        store.create_session().await.unwrap();

        let sessions = store.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 2);
    }
}
