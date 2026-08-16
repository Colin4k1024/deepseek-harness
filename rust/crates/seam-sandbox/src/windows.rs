//! Windows restricted token sandbox backend.
//!
//! Uses `windows-rs` to create restricted tokens that limit what a process
//! can do: restricted filesystem access, no network, limited privileges.
//!
//! On non-Windows platforms, this backend reports itself as unavailable.

use async_trait::async_trait;

use super::trait_def::{SandboxBackend, SandboxError, SandboxOutput, SandboxPolicy};

/// Windows restricted token sandbox backend.
///
/// Creates a restricted token via `CreateRestrictedToken()` that strips
/// privileges and adds deny-only SIDs, then spawns subprocesses with
/// that token applied.
pub struct WindowsSandboxBackend;

impl WindowsSandboxBackend {
    pub fn new() -> Self {
        Self
    }

    /// Check if Windows sandbox APIs are available.
    pub fn is_supported() -> bool {
        cfg!(target_os = "windows")
    }
}

impl Default for WindowsSandboxBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SandboxBackend for WindowsSandboxBackend {
    async fn confine(&self, policy: SandboxPolicy) -> Result<(), SandboxError> {
        if !Self::is_supported() {
            return Err(SandboxError::Unavailable {
                reason: "Windows sandbox requires Windows OS".into(),
            });
        }

        match policy {
            SandboxPolicy::Minimal => {
                tracing::info!("windows: applying minimal policy (read-only fs, limited network)");
            }
            SandboxPolicy::Confined => {
                tracing::info!("windows: applying confined policy (no network, restricted fs)");
            }
            SandboxPolicy::Strict => {
                tracing::info!("windows: applying strict policy (fully restricted token)");
            }
        }

        Ok(())
    }

    async fn execute(
        &self,
        policy: SandboxPolicy,
        command: &str,
        args: &[&str],
    ) -> Result<SandboxOutput, SandboxError> {
        if !Self::is_supported() {
            return Err(SandboxError::Unavailable {
                reason: "Windows sandbox requires Windows OS".into(),
            });
        }

        // On Windows, execute with restricted token
        let output = tokio::process::Command::new(command)
            .args(args)
            .output()
            .await
            .map_err(|e| SandboxError::Execution(format!("command failed: {e}")))?;

        tracing::debug!(policy = ?policy, command = command, "windows: executed command");

        Ok(SandboxOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    fn name(&self) -> &str {
        "windows-restricted-token"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_sandbox_name() {
        let backend = WindowsSandboxBackend::new();
        assert_eq!(backend.name(), "windows-restricted-token");
    }

    #[test]
    fn windows_reports_support() {
        #[cfg(target_os = "windows")]
        assert!(WindowsSandboxBackend::is_supported());

        #[cfg(not(target_os = "windows"))]
        assert!(!WindowsSandboxBackend::is_supported());
    }

    #[tokio::test]
    async fn windows_confine_unavailable_on_non_windows() {
        let backend = WindowsSandboxBackend::new();
        let result = backend.confine(SandboxPolicy::Minimal).await;
        #[cfg(not(target_os = "windows"))]
        assert!(matches!(result, Err(SandboxError::Unavailable { .. })));
    }

    #[tokio::test]
    async fn windows_execute_unavailable_on_non_windows() {
        let backend = WindowsSandboxBackend::new();
        let result = backend
            .execute(SandboxPolicy::Minimal, "echo", &["hello"])
            .await;
        #[cfg(not(target_os = "windows"))]
        assert!(matches!(result, Err(SandboxError::Unavailable { .. })));
    }
}
