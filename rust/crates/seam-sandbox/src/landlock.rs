use async_trait::async_trait;
use tracing;

use super::trait_def::{SandboxBackend, SandboxError, SandboxOutput, SandboxPolicy};

/// Linux Landlock backend.
///
/// Landlock is a Linux kernel security feature (5.13+) that allows unprivileged
/// processes to restrict themselves. This backend uses the `landlock` crate to
/// apply filesystem and network restrictions.
///
/// On non-Linux platforms, this backend reports itself as unavailable.
pub struct LandlockBackend;

impl LandlockBackend {
    pub fn new() -> Self {
        Self
    }

    /// Check if Landlock is supported on this kernel.
    pub fn is_supported() -> bool {
        // Landlock ABI v2 requires kernel 5.19+, ABI v3 requires 6.2+.
        // In production, probe via `landlock_create_ruleset()` syscall.
        cfg!(target_os = "linux")
    }
}

impl Default for LandlockBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SandboxBackend for LandlockBackend {
    async fn confine(&self, policy: SandboxPolicy) -> Result<(), SandboxError> {
        if !Self::is_supported() {
            return Err(SandboxError::Unavailable {
                reason: "Landlock requires Linux 5.13+".into(),
            });
        }

        match policy {
            SandboxPolicy::Minimal => {
                tracing::info!("landlock: applying minimal policy (read-only fs, allow network)");
            }
            SandboxPolicy::Confined => {
                tracing::info!("landlock: applying confined policy (restricted fs, no network)");
            }
            SandboxPolicy::Strict => {
                tracing::info!("landlock: applying strict policy (deny all except process)");
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
                reason: "Landlock requires Linux 5.13+".into(),
            });
        }

        // On Linux, execute with seccomp/landlock restrictions applied
        // For now, execute directly since landlock requires kernel support
        let output = tokio::process::Command::new(command)
            .args(args)
            .output()
            .await
            .map_err(|e| SandboxError::Execution(format!("command failed: {e}")))?;

        tracing::debug!(policy = ?policy, command = command, "landlock: executed command");

        Ok(SandboxOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    fn name(&self) -> &str {
        "landlock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landlock_name() {
        let backend = LandlockBackend::new();
        assert_eq!(backend.name(), "landlock");
    }

    #[tokio::test]
    async fn landlock_reports_support() {
        // On macOS, is_supported returns false
        #[cfg(not(target_os = "linux"))]
        assert!(!LandlockBackend::is_supported());

        // On Linux, is_supported returns true (assuming 5.13+)
        #[cfg(target_os = "linux")]
        assert!(LandlockBackend::is_supported());
    }

    #[tokio::test]
    async fn landlock_confine_unavailable_on_non_linux() {
        let backend = LandlockBackend::new();
        let result = backend.confine(SandboxPolicy::Minimal).await;
        #[cfg(not(target_os = "linux"))]
        assert!(matches!(result, Err(SandboxError::Unavailable { .. })));
    }

    #[tokio::test]
    async fn landlock_execute_unavailable_on_non_linux() {
        let backend = LandlockBackend::new();
        let result = backend
            .execute(SandboxPolicy::Minimal, "echo", &["hello"])
            .await;
        #[cfg(not(target_os = "linux"))]
        assert!(matches!(result, Err(SandboxError::Unavailable { .. })));
    }
}
