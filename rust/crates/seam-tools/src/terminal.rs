//! Terminal tool — persistent shell sessions with working directory tracking.
//!
//! Each terminal session has a unique ID and maintains its own working
//! directory. Commands are executed via `bash -c` in the session's
//! working directory. `cd` commands update the working directory.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};

/// A persistent terminal session.
struct TerminalSession {
    id: String,
    working_dir: PathBuf,
}

/// Terminal tool that manages persistent shell sessions.
///
/// Sessions persist across tool invocations. Each session tracks its
/// working directory. The `cd` command is intercepted to update the
/// working directory rather than spawning a new shell.
pub struct TerminalTool {
    sessions: Mutex<HashMap<String, TerminalSession>>,
    next_id: Mutex<u64>,
    default_shell: String,
    command_timeout_secs: u64,
}

impl TerminalTool {
    /// Create a new terminal tool.
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            next_id: Mutex::new(1),
            default_shell: "bash".into(),
            command_timeout_secs: 30,
        }
    }

    /// Set the shell to use for command execution.
    pub fn with_shell(mut self, shell: impl Into<String>) -> Self {
        self.default_shell = shell.into();
        self
    }

    /// Set the command timeout in seconds.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.command_timeout_secs = secs;
        self
    }

    /// Create a new session and return its ID.
    fn create_session(&self) -> String {
        let mut next = self.next_id.lock().unwrap();
        let id = format!("term-{}", *next);
        *next += 1;

        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(
            id.clone(),
            TerminalSession {
                id: id.clone(),
                working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            },
        );
        id
    }

    /// List all active sessions.
    fn list_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .values()
            .map(|s| format!("{} ({})", s.id, s.working_dir.display()))
            .collect()
    }

    /// Destroy a session.
    fn destroy_session(&self, id: &str) -> bool {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.remove(id).is_some()
    }

    /// Execute a command in a session.
    fn execute_in_session(&self, session_id: &str, command: &str) -> Result<String, ToolError> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| ToolError::Execution(format!("session '{session_id}' not found")))?;

        // Handle cd specially
        if command.trim().starts_with("cd ") {
            let dir = command.trim()[3..].trim().trim_matches('"').trim_matches('\'');
            let new_dir = if dir == "~" || dir == "$HOME" {
                dirs_home()
            } else if dir.starts_with('/') {
                PathBuf::from(dir)
            } else {
                session.working_dir.join(dir)
            };

            if new_dir.exists() && new_dir.is_dir() {
                session.working_dir = new_dir.canonicalize().unwrap_or(new_dir);
                Ok(format!("Changed to {}", session.working_dir.display()))
            } else {
                Ok(format!("Directory not found: {}", new_dir.display()))
            }
        } else {
            // Execute the command
            let output = std::process::Command::new(&self.default_shell)
                .arg("-c")
                .arg(command)
                .current_dir(&session.working_dir)
                .env_remove("CDPATH")
                .output()
                .map_err(|e| ToolError::Execution(format!("command failed: {e}")))?;

            let mut result = String::new();
            if !output.stdout.is_empty() {
                result.push_str(&String::from_utf8_lossy(&output.stdout));
            }
            if !output.stderr.is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(&format!(
                    "[stderr]\n{}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            if !output.status.success() {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(&format!(
                    "[exit code: {}]",
                    output.status.code().unwrap_or(-1)
                ));
            }
            if result.is_empty() {
                result.push_str("[no output]");
            }
            Ok(result)
        }
    }
}

impl Default for TerminalTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the home directory.
fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"))
}

#[async_trait]
impl Tool for TerminalTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "terminal".into(),
            description: "Execute commands in a persistent terminal session. \
                Each session maintains its own working directory. \
                Use 'create' to start a new session, 'exec' to run a command, \
                'list' to see all sessions, and 'destroy' to end a session. \
                The 'cd' command is intercepted to update the working directory \
                without spawning a subshell.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["create", "exec", "list", "destroy"],
                        "description": "The action: create a new session, exec a command, list sessions, or destroy a session"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "The session ID (required for exec and destroy)"
                    },
                    "command": {
                        "type": "string",
                        "description": "The command to execute (required for exec)"
                    }
                },
                "required": ["action"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("action is required".into()))?;

        match action {
            "create" => {
                let id = self.create_session();
                Ok(ToolResult {
                    content: format!("Terminal session created: {id}"),
                    is_error: false,
                })
            }
            "list" => {
                let sessions = self.list_sessions();
                if sessions.is_empty() {
                    Ok(ToolResult {
                        content: "No active terminal sessions. Use action='create' to start one."
                            .into(),
                        is_error: false,
                    })
                } else {
                    Ok(ToolResult {
                        content: format!("Active sessions:\n{}", sessions.join("\n")),
                        is_error: false,
                    })
                }
            }
            "destroy" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidArgs("session_id is required for destroy".into())
                    })?;
                if self.destroy_session(session_id) {
                    Ok(ToolResult {
                        content: format!("Session '{session_id}' destroyed."),
                        is_error: false,
                    })
                } else {
                    Ok(ToolResult {
                        content: format!("Session '{session_id}' not found."),
                        is_error: true,
                    })
                }
            }
            "exec" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidArgs("session_id is required for exec".into())
                    })?;
                let command = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidArgs("command is required for exec".into())
                    })?;

                match self.execute_in_session(session_id, command) {
                    Ok(output) => Ok(ToolResult {
                        content: output,
                        is_error: false,
                    }),
                    Err(e) => Ok(ToolResult {
                        content: format!("{e}"),
                        is_error: true,
                    }),
                }
            }
            _ => Ok(ToolResult {
                content: format!("Unknown action: '{action}'. Use create, exec, list, or destroy."),
                is_error: true,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_and_list() {
        let tool = TerminalTool::new();
        let result = tool.execute(serde_json::json!({"action": "create"})).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("created"));

        let result = tool.execute(serde_json::json!({"action": "list"})).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("term-1"));
    }

    #[tokio::test]
    async fn create_and_destroy() {
        let tool = TerminalTool::new();
        tool.execute(serde_json::json!({"action": "create"})).await.unwrap();

        let result = tool
            .execute(serde_json::json!({"action": "destroy", "session_id": "term-1"}))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("destroyed"));

        let result = tool.execute(serde_json::json!({"action": "list"})).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("No active"));
    }

    #[tokio::test]
    async fn exec_in_session() {
        let tool = TerminalTool::new();
        tool.execute(serde_json::json!({"action": "create"})).await.unwrap();

        let result = tool
            .execute(serde_json::json!({
                "action": "exec",
                "session_id": "term-1",
                "command": "echo hello from terminal"
            }))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("hello from terminal"));
    }

    #[tokio::test]
    async fn cd_changes_directory() {
        let tool = TerminalTool::new();
        tool.execute(serde_json::json!({"action": "create"})).await.unwrap();

        // cd to /tmp
        let result = tool
            .execute(serde_json::json!({
                "action": "exec",
                "session_id": "term-1",
                "command": "cd /tmp"
            }))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("Changed"));

        // pwd should show /tmp
        let result = tool
            .execute(serde_json::json!({
                "action": "exec",
                "session_id": "term-1",
                "command": "pwd"
            }))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("/tmp"));
    }

    #[tokio::test]
    async fn exec_nonexistent_session() {
        let tool = TerminalTool::new();
        let result = tool
            .execute(serde_json::json!({
                "action": "exec",
                "session_id": "nonexistent",
                "command": "echo hi"
            }))
            .await
            .unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("not found"));
    }

    #[tokio::test]
    async fn missing_action() {
        let tool = TerminalTool::new();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn destroy_nonexistent() {
        let tool = TerminalTool::new();
        let result = tool
            .execute(serde_json::json!({"action": "destroy", "session_id": "nonexistent"}))
            .await
            .unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("not found"));
    }

    #[tokio::test]
    async fn schema_check() {
        let tool = TerminalTool::new();
        let schema = tool.schema();
        assert_eq!(schema.name, "terminal");
        assert!(schema.description.contains("persistent"));
        let params = schema.parameters.as_object().unwrap();
        let props = params.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("action"));
        assert!(props.contains_key("session_id"));
        assert!(props.contains_key("command"));
    }
}