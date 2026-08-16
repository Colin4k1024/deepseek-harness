use async_trait::async_trait;
use thiserror::Error;

/// Errors from subprocess operations.
#[derive(Debug, Error)]
pub enum SubprocessError {
    #[error("process spawn failed: {0}")]
    Spawn(String),
    #[error("process exited with code {0}")]
    Exited(i32),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result of a subprocess execution.
#[derive(Debug)]
pub struct SubprocessResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
}

/// Trait for subprocess capability providers.
#[async_trait]
pub trait Subprocess: Send + Sync {
    /// Spawn a process and wait for completion.
    async fn exec(&self, program: &str, args: &[&str])
    -> Result<SubprocessResult, SubprocessError>;

    /// Spawn a process in the background, returning a handle.
    async fn spawn(&self, program: &str, args: &[&str]) -> Result<u32, SubprocessError>;
}
