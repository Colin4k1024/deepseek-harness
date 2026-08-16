//! Authentication — API key management with hashed storage.

use std::collections::HashMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Authentication errors.
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("invalid API key")]
    InvalidKey,
    #[error("key expired")]
    Expired,
    #[error("insufficient permissions")]
    Forbidden,
    #[error("rate limit exceeded")]
    RateLimited,
    #[error("quota exceeded")]
    QuotaExceeded,
}

/// Credentials for authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    /// API key (plaintext, only used for verification).
    pub api_key: String,
}

/// An API key entry with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyEntry {
    /// SHA-256 hash of the key.
    pub key_hash: String,

    /// Key name/description.
    pub name: String,

    /// Associated user/tenant ID.
    pub user_id: String,

    /// Permissions (e.g., ["chat", "admin"]).
    pub permissions: Vec<String>,

    /// Whether the key is active.
    pub active: bool,

    /// Creation timestamp (Unix seconds).
    pub created_at: u64,

    /// Expiration timestamp (Unix seconds, 0 = never).
    pub expires_at: u64,
}

/// Authentication manager — verifies API keys and enforces permissions.
pub struct AuthManager {
    keys: RwLock<HashMap<String, ApiKeyEntry>>,
}

impl AuthManager {
    /// Create a new empty auth manager.
    pub fn new() -> Self {
        Self {
            keys: RwLock::new(HashMap::new()),
        }
    }

    /// Register an API key.
    pub fn register_key(&self, key: &str, name: &str, user_id: &str, permissions: Vec<String>) {
        let hash = hash_key(key);
        let entry = ApiKeyEntry {
            key_hash: hash.clone(),
            name: name.into(),
            user_id: user_id.into(),
            permissions,
            active: true,
            created_at: now(),
            expires_at: 0,
        };
        self.keys.write().unwrap().insert(hash, entry);
    }

    /// Register an API key with expiration.
    pub fn register_key_with_expiry(
        &self,
        key: &str,
        name: &str,
        user_id: &str,
        permissions: Vec<String>,
        expires_at: u64,
    ) {
        let hash = hash_key(key);
        let entry = ApiKeyEntry {
            key_hash: hash.clone(),
            name: name.into(),
            user_id: user_id.into(),
            permissions,
            active: true,
            created_at: now(),
            expires_at,
        };
        self.keys.write().unwrap().insert(hash, entry);
    }

    /// Verify an API key and return the associated entry.
    pub fn verify(&self, key: &str) -> Result<ApiKeyEntry, AuthError> {
        let hash = hash_key(key);
        let keys = self.keys.read().unwrap();

        let entry = keys.get(&hash).ok_or(AuthError::InvalidKey)?;

        if !entry.active {
            return Err(AuthError::InvalidKey);
        }

        if entry.expires_at > 0 && now() > entry.expires_at {
            return Err(AuthError::Expired);
        }

        Ok(entry.clone())
    }

    /// Check if a key has a specific permission.
    pub fn check_permission(&self, key: &str, permission: &str) -> Result<(), AuthError> {
        let entry = self.verify(key)?;
        if entry.permissions.contains(&"*".to_string())
            || entry.permissions.contains(&permission.to_string())
        {
            Ok(())
        } else {
            Err(AuthError::Forbidden)
        }
    }

    /// Revoke an API key.
    pub fn revoke(&self, key: &str) {
        let hash = hash_key(key);
        if let Some(entry) = self.keys.write().unwrap().get_mut(&hash) {
            entry.active = false;
        }
    }

    /// List all active keys (admin operation).
    pub fn list_keys(&self) -> Vec<ApiKeyEntry> {
        self.keys
            .read()
            .unwrap()
            .values()
            .filter(|e| e.active)
            .cloned()
            .collect()
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Hash an API key using SHA-256.
pub fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_verify() {
        let auth = AuthManager::new();
        auth.register_key("sk-test-123", "test key", "user1", vec!["chat".into()]);

        let entry = auth.verify("sk-test-123").unwrap();
        assert_eq!(entry.user_id, "user1");
        assert!(entry.active);
    }

    #[test]
    fn invalid_key() {
        let auth = AuthManager::new();
        assert!(matches!(auth.verify("invalid"), Err(AuthError::InvalidKey)));
    }

    #[test]
    fn revoked_key() {
        let auth = AuthManager::new();
        auth.register_key("sk-test", "test", "user1", vec!["chat".into()]);
        auth.revoke("sk-test");
        assert!(matches!(auth.verify("sk-test"), Err(AuthError::InvalidKey)));
    }

    #[test]
    fn permission_check() {
        let auth = AuthManager::new();
        auth.register_key("sk-admin", "admin", "user1", vec!["*".into()]);
        auth.register_key("sk-user", "user", "user2", vec!["chat".into()]);

        assert!(auth.check_permission("sk-admin", "admin").is_ok());
        assert!(auth.check_permission("sk-user", "chat").is_ok());
        assert!(matches!(
            auth.check_permission("sk-user", "admin"),
            Err(AuthError::Forbidden)
        ));
    }

    #[test]
    fn expired_key() {
        let auth = AuthManager::new();
        auth.register_key_with_expiry(
            "sk-expired",
            "expired",
            "user1",
            vec!["chat".into()],
            1000, // already expired
        );

        assert!(matches!(auth.verify("sk-expired"), Err(AuthError::Expired)));
    }

    #[test]
    fn hash_deterministic() {
        let h1 = hash_key("sk-test");
        let h2 = hash_key("sk-test");
        assert_eq!(h1, h2);

        let h3 = hash_key("sk-different");
        assert_ne!(h1, h3);
    }
}
