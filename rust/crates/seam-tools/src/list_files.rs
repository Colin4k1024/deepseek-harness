//! List files tool — list directory contents.

use async_trait::async_trait;
use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};
use serde_json::json;

/// Tool for listing directory contents.
pub struct ListFilesTool;

#[async_trait]
impl Tool for ListFilesTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "list_files".into(),
            description: "List files and directories in a given path".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory path to list (default: current directory)"
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let path = args["path"].as_str().unwrap_or(".");

        let entries = tokio::fs::read_dir(path)
            .await
            .map_err(|e| ToolError::Execution(format!("failed to read directory '{path}': {e}")))?;

        let mut result = String::new();
        let mut entries = entries;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| ToolError::Execution(format!("failed to read entry: {e}")))?
        {
            let file_type = entry
                .file_type()
                .await
                .map_err(|e| ToolError::Execution(format!("failed to get file type: {e}")))?;

            let name = entry.file_name().to_string_lossy().to_string();
            let prefix = if file_type.is_dir() { "d " } else { "f " };
            result.push_str(&format!("{prefix}{name}\n"));
        }

        if result.is_empty() {
            result = "(empty directory)".into();
        }

        Ok(ToolResult {
            content: result,
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn list_current_directory() {
        let tool = ListFilesTool;
        let result = tool.execute(json!({"path": "."})).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("Cargo.toml"));
    }

    #[tokio::test]
    async fn list_nonexistent_directory() {
        let tool = ListFilesTool;
        let result = tool.execute(json!({"path": "/nonexistent"})).await;
        assert!(result.is_err());
    }
}
