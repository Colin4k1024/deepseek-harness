use super::trait_def::{Subprocess, SubprocessError, SubprocessResult};
use async_trait::async_trait;

/// Local process-tree subprocess provider.
pub struct LocalSubprocess;

impl LocalSubprocess {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LocalSubprocess {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Subprocess for LocalSubprocess {
    async fn exec(
        &self,
        program: &str,
        args: &[&str],
    ) -> Result<SubprocessResult, SubprocessError> {
        let output = tokio::process::Command::new(program)
            .args(args)
            .output()
            .await?;

        Ok(SubprocessResult {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    async fn spawn(&self, program: &str, args: &[&str]) -> Result<u32, SubprocessError> {
        let child = tokio::process::Command::new(program).args(args).spawn()?;
        Ok(child.id().unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn exec_echo() {
        let sp = LocalSubprocess::new();
        let result = sp.exec("echo", &["hello"]).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "hello");
    }

    #[tokio::test]
    async fn exec_failing_command() {
        let sp = LocalSubprocess::new();
        let result = sp.exec("false", &[]).await.unwrap();
        assert_ne!(result.exit_code, 0);
    }

    #[tokio::test]
    async fn spawn_returns_pid() {
        let sp = LocalSubprocess::new();
        let pid = sp.spawn("sleep", &["1"]).await.unwrap();
        assert!(pid > 0);
    }
}
