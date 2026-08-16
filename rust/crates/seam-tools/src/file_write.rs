//! File write tool — write file contents.

use async_trait::async_trait;
use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};
use serde_json::json;

/// Tool for writing file contents.
pub struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "write_file".into(),
            description: "Write content to a file".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write"
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("missing 'path' field".into()))?;
        let content = args["content"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("missing 'content' field".into()))?;

        tokio::fs::write(path, content)
            .await
            .map_err(|e| ToolError::Execution(format!("failed to write file '{path}': {e}")))?;

        Ok(ToolResult {
            content: format!("Successfully wrote {} bytes to {path}", content.len()),
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn write_and_read_back() {
        let tool = FileWriteTool;
        let tmp_path = "/tmp/dsh_test_write.txt";
        let test_content = "hello from dsh";

        let result = tool
            .execute(json!({"path": tmp_path, "content": test_content}))
            .await
            .unwrap();
        assert!(!result.is_error);

        let read_back = tokio::fs::read_to_string(tmp_path).await.unwrap();
        assert_eq!(read_back, test_content);

        // Cleanup
        let _ = tokio::fs::remove_file(tmp_path).await;
    }
}
