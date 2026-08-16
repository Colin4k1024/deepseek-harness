use async_trait::async_trait;
use thiserror::Error;

/// Errors from filesystem operations.
#[derive(Debug, Error)]
pub enum FsError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("path not found: {0}")]
    NotFound(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
}

/// Trait for filesystem capability providers.
#[async_trait]
pub trait FileSystem: Send + Sync {
    /// Read a file's contents as bytes.
    async fn read(&self, path: &str) -> Result<Vec<u8>, FsError>;

    /// Write bytes to a file.
    async fn write(&self, path: &str, contents: &[u8]) -> Result<(), FsError>;

    /// List entries in a directory.
    async fn list(&self, path: &str) -> Result<Vec<String>, FsError>;

    /// Check if a path exists.
    async fn exists(&self, path: &str) -> Result<bool, FsError>;
}
