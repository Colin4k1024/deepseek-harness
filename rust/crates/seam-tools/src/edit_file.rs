//! Edit file tool — partial file editing with search/replace.

use async_trait::async_trait;
use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};
use serde_json::json;

/// Tool for editing files with search/replace.
pub struct EditFileTool;

#[async_trait]
impl Tool for EditFileTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "edit_file".into(),
            description: "Edit a file by replacing occurrences of a search string with a replacement string".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to edit"
                    },
                    "search": {
                        "type": "string",
                        "description": "String to search for"
                    },
                    "replace": {
                        "type": "string",
                        "description": "Replacement string"
                    }
                },
                "required": ["path", "search", "replace"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("missing 'path' field".into()))?;
        let search = args["search"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("missing 'search' field".into()))?;
        let replace = args["replace"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("missing 'replace' field".into()))?;

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ToolError::Execution(format!("failed to read file '{path}': {e}")))?;

        let count = content.matches(search).count();
        if count == 0 {
            return Ok(ToolResult {
                content: format!("No occurrences of '{search}' found in {path}"),
                is_error: false,
            });
        }

        let new_content = content.replace(search, replace);
        tokio::fs::write(path, &new_content)
            .await
            .map_err(|e| ToolError::Execution(format!("failed to write file '{path}': {e}")))?;

        Ok(ToolResult {
            content: format!("Replaced {count} occurrence(s) of '{search}' in {path}"),
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn edit_file_replace() {
        let tmp = "/tmp/dsh_edit_test.txt";
        tokio::fs::write(tmp, "hello world\nhello rust\n").await.unwrap();

        let tool = EditFileTool;
        let result = tool
            .execute(json!({"path": tmp, "search": "hello", "replace": "hi"}))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("2"));

        let content = tokio::fs::read_to_string(tmp).await.unwrap();
        assert_eq!(content, "hi world\nhi rust\n");

        let _ = tokio::fs::remove_file(tmp).await;
    }

    #[tokio::test]
    async fn edit_file_no_match() {
        let tmp = "/tmp/dsh_edit_nomatch.txt";
        tokio::fs::write(tmp, "hello world").await.unwrap();

        let tool = EditFileTool;
        let result = tool
            .execute(json!({"path": tmp, "search": "xyz", "replace": "abc"}))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("No occurrences"));

        let _ = tokio::fs::remove_file(tmp).await;
    }
}
