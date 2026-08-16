//! Bash tool — execute shell commands.

use async_trait::async_trait;
use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};
use serde_json::json;

/// Tool for executing bash commands.
pub struct BashTool {
    #[allow(dead_code)]
    working_dir: Option<String>,
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

impl BashTool {
    pub fn new() -> Self {
        Self { working_dir: None }
    }

    pub fn with_working_dir(mut self, dir: String) -> Self {
        self.working_dir = Some(dir);
        self
    }
}

#[async_trait]
impl Tool for BashTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "bash".into(),
            description: "Execute a bash command and return its output".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let command = args["command"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("missing 'command' field".into()))?;

        let output = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(command)
            .output()
            .await
            .map_err(|e| ToolError::Execution(format!("failed to execute command: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let content = if output.status.success() {
            if stdout.is_empty() {
                "(no output)".into()
            } else {
                stdout
            }
        } else {
            format!(
                "stdout: {stdout}\nstderr: {stderr}\nexit code: {}",
                output.status.code().unwrap_or(-1)
            )
        };

        Ok(ToolResult {
            content,
            is_error: !output.status.success(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn execute_simple_command() {
        let tool = BashTool::new();
        let result = tool
            .execute(json!({"command": "echo hello"}))
            .await
            .unwrap();
        assert_eq!(result.content.trim(), "hello");
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn execute_failing_command() {
        let tool = BashTool::new();
        let result = tool.execute(json!({"command": "false"})).await.unwrap();
        assert!(result.is_error);
    }
}
