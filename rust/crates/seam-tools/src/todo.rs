//! Todo tool — records and manages a structured task list.
//!
//! Each call REPLACES the entire task list. Tasks have `content` (text)
//! and `status` (pending | in_progress | completed).

use async_trait::async_trait;
use std::sync::Mutex;

use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};

/// A single todo item.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TodoItem {
    pub content: String,
    pub status: String,
}

/// Tool that manages a structured task list.
///
/// The entire list is replaced on each call. Statuses are:
/// `pending` (not started), `in_progress` (being worked on), `completed` (done).
pub struct TodoTool {
    todos: Mutex<Vec<TodoItem>>,
}

impl TodoTool {
    /// Create a new todo tool with an empty list.
    pub fn new() -> Self {
        Self {
            todos: Mutex::new(Vec::new()),
        }
    }

    /// Return the current todo list.
    pub fn list(&self) -> Vec<TodoItem> {
        self.todos.lock().unwrap().clone()
    }

    /// Replace the todo list.
    pub fn replace(&self, items: Vec<TodoItem>) {
        let mut todos = self.todos.lock().unwrap();
        *todos = items;
    }
}

impl Default for TodoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TodoTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "todo_write".into(),
            description: "Record and update a structured task list for the current work. \
                Send the ENTIRE list every call — it REPLACES the previous list. \
                Use it to plan multi-step work and show progress. \
                Statuses: pending (not started), in_progress (being worked on now), \
                completed (done).".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "description": "The COMPLETE task list, replacing any previous list.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "content": {
                                    "type": "string",
                                    "description": "What the task is — a short imperative line."
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed"],
                                    "description": "pending | in_progress | completed"
                                }
                            },
                            "required": ["content", "status"]
                        }
                    }
                },
                "required": ["todos"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let todos = args
            .get("todos")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ToolError::InvalidArgs("todos array is required".into()))?;

        // Validate each todo
        let mut items = Vec::new();
        for (i, todo) in todos.iter().enumerate() {
            let content = todo
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ToolError::InvalidArgs(format!("todo[{i}]: content is required"))
                })?;
            let status = todo
                .get("status")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ToolError::InvalidArgs(format!("todo[{i}]: status is required"))
                })?;
            if !["pending", "in_progress", "completed"].contains(&status) {
                return Err(ToolError::InvalidArgs(format!(
                    "todo[{i}]: invalid status '{status}' — must be pending, in_progress, or completed"
                )));
            }
            items.push(TodoItem {
                content: content.to_string(),
                status: status.to_string(),
            });
        }

        self.replace(items);

        // Return a summary
        let current = self.list();
        let summary = if current.is_empty() {
            "Todo list is empty.".to_string()
        } else {
            let mut lines = vec![format!("Todo list ({} items):", current.len())];
            for item in &current {
                let icon = match item.status.as_str() {
                    "completed" => "✅",
                    "in_progress" => "🔄",
                    _ => "⬜",
                };
                lines.push(format!("  {icon} {status_label} — {content}",
                    content = item.content,
                    status_label = match item.status.as_str() {
                        "completed" => "done",
                        "in_progress" => "in progress",
                        _ => "pending",
                    },
                    icon = icon,
                ));
            }
            lines.join("\n")
        };

        Ok(ToolResult {
            content: summary,
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn todo_write_and_replace() {
        let tool = TodoTool::new();

        // Write initial list
        let result = tool
            .execute(serde_json::json!({
                "todos": [
                    {"content": "Task A", "status": "in_progress"},
                    {"content": "Task B", "status": "pending"}
                ]
            }))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("Task A"));
        assert!(result.content.contains("Task B"));

        let list = tool.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].status, "in_progress");
        assert_eq!(list[1].status, "pending");
    }

    #[tokio::test]
    async fn todo_replace_clears_old() {
        let tool = TodoTool::new();

        // Write initial list
        tool.execute(serde_json::json!({
            "todos": [
                {"content": "Task A", "status": "pending"}
            ]
        }))
        .await
        .unwrap();

        // Replace with new list
        tool.execute(serde_json::json!({
            "todos": [
                {"content": "Task C", "status": "completed"}
            ]
        }))
        .await
        .unwrap();

        let list = tool.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].content, "Task C");
        assert_eq!(list[0].status, "completed");
    }

    #[tokio::test]
    async fn todo_empty_list() {
        let tool = TodoTool::new();
        let result = tool
            .execute(serde_json::json!({"todos": []}))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("empty"));
        assert!(tool.list().is_empty());
    }

    #[tokio::test]
    async fn todo_invalid_status() {
        let tool = TodoTool::new();
        let result = tool
            .execute(serde_json::json!({
                "todos": [
                    {"content": "Task A", "status": "invalid"}
                ]
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn todo_missing_todos() {
        let tool = TodoTool::new();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn todo_missing_content() {
        let tool = TodoTool::new();
        let result = tool
            .execute(serde_json::json!({
                "todos": [{"status": "pending"}]
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn todo_all_statuses() {
        let tool = TodoTool::new();
        let result = tool
            .execute(serde_json::json!({
                "todos": [
                    {"content": "Pending", "status": "pending"},
                    {"content": "In Progress", "status": "in_progress"},
                    {"content": "Done", "status": "completed"}
                ]
            }))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("Pending"));
        assert!(result.content.contains("In Progress"));
        assert!(result.content.contains("Done"));
    }

    #[test]
    fn schema_is_valid() {
        let tool = TodoTool::new();
        let schema = tool.schema();
        assert_eq!(schema.name, "todo_write");
        assert!(schema.description.contains("REPLACES"));
        let params = schema.parameters.as_object().unwrap();
        assert!(params.contains_key("required"));
    }
}