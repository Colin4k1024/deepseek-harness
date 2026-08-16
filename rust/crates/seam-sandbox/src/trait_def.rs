use async_trait::async_trait;
use thiserror::Error;

/// Errors from sandbox operations.
#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("sandbox unavailable: {reason}")]
    Unavailable { reason: String },
    #[error("sandbox configuration error: {0}")]
    Config(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("command execution failed: {0}")]
    Execution(String),
}

/// Sandbox policy level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxPolicy {
    /// Minimal restrictions — network + filesystem read-only.
    Minimal,
    /// Confined — no network, restricted filesystem.
    Confined,
    /// Strict — no network, no filesystem, minimal syscall surface.
    Strict,
}

/// Result of a sandboxed command execution.
#[derive(Debug)]
pub struct SandboxOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
}

/// Trait for sandbox backends.
#[async_trait]
pub trait SandboxBackend: Send + Sync {
    /// Apply the sandbox policy to the current process or a subprocess.
    async fn confine(&self, policy: SandboxPolicy) -> Result<(), SandboxError>;

    /// Execute a command in a sandboxed environment.
    async fn execute(
        &self,
        policy: SandboxPolicy,
        command: &str,
        args: &[&str],
    ) -> Result<SandboxOutput, SandboxError>;

    /// Return the name of this sandbox backend.
    fn name(&self) -> &str;
}
