use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors from tool execution.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("tool execution failed: {0}")]
    Execution(String),
    #[error("invalid arguments: {0}")]
    InvalidArgs(String),
}

/// JSON Schema for tool arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
}

/// Trait for tools that can be invoked by the agent loop.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Return the tool's schema (name, description, parameters).
    fn schema(&self) -> ToolSchema;

    /// Execute the tool with the given arguments.
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError>;
}
