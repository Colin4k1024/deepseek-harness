//! Search tool — search for files by pattern.

use async_trait::async_trait;
use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};
use serde_json::json;

/// Tool for searching files by pattern.
pub struct SearchTool;

#[async_trait]
impl Tool for SearchTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "search_files".into(),
            description: "Search for files matching a glob pattern".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern to match (e.g. '*.rs', '**/*.toml')"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory to search in (default: current directory)"
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let pattern = args["pattern"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("missing 'pattern' field".into()))?;
        let base_path = args["path"].as_str().unwrap_or(".");

        // Use find command for glob matching
        let output = tokio::process::Command::new("find")
            .arg(base_path)
            .arg("-name")
            .arg(pattern)
            .arg("-type")
            .arg("f")
            .output()
            .await
            .map_err(|e| ToolError::Execution(format!("search failed: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let files: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

        if files.is_empty() {
            Ok(ToolResult {
                content: format!("No files found matching '{pattern}' in {base_path}"),
                is_error: false,
            })
        } else {
            let content = format!("Found {} files:\n{}", files.len(), files.join("\n"));
            Ok(ToolResult {
                content,
                is_error: false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn search_for_cargo_toml() {
        let tool = SearchTool;
        let result = tool
            .execute(json!({"pattern": "Cargo.toml", "path": "."}))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("Cargo.toml"));
    }

    #[tokio::test]
    async fn search_nonexistent_pattern() {
        let tool = SearchTool;
        let result = tool
            .execute(json!({"pattern": "*.nonexistent_ext_12345", "path": "."}))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("No files found"));
    }
}
