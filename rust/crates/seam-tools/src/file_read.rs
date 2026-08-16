//! File read tool — read file contents.

use async_trait::async_trait;
use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};
use serde_json::json;

/// Tool for reading file contents.
pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "read_file".into(),
            description: "Read the contents of a file".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to read"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("missing 'path' field".into()))?;

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ToolError::Execution(format!("failed to read file '{path}': {e}")))?;

        Ok(ToolResult {
            content,
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn read_existing_file() {
        let tool = FileReadTool;
        let result = tool.execute(json!({"path": "Cargo.toml"})).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("[package]"));
    }

    #[tokio::test]
    async fn read_nonexistent_file() {
        let tool = FileReadTool;
        let result = tool.execute(json!({"path": "/nonexistent/file.txt"})).await;
        assert!(result.is_err());
    }
}
