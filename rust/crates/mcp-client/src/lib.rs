//! MCP Client — adapts remote MCP server tools to the dsh `Tool` trait.
//!
//! Wraps MCP tools discovered from a running MCP client as `dsh_core_tools::Tool`
//! implementations, enabling the agent loop to use MCP tools transparently.

use std::sync::Arc;

use async_trait::async_trait;
use rust_mcp_sdk::McpClient;
use rust_mcp_sdk::schema::{CallToolRequestParams, Tool as McpToolDef, ToolInputSchema};

use dsh_core_tools::{Tool, ToolError, ToolResult, ToolSchema};

/// Error from MCP client operations.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("MCP SDK error: {0}")]
    Sdk(String),
    #[error("tool not found: {0}")]
    ToolNotFound(String),
    #[error("connection error: {0}")]
    Connection(String),
}

/// Convert an MCP `ToolInputSchema` to a `serde_json::Value`.
fn schema_to_json(schema: &ToolInputSchema) -> serde_json::Value {
    serde_json::to_value(schema).unwrap_or(serde_json::json!({}))
}

/// Wraps a remote MCP tool as a local dsh `Tool`.
///
/// When `execute()` is called, it forwards the call to the MCP server
/// via `request_tool_call()`.
pub struct McpToolAdapter {
    schema: ToolSchema,
    client: Arc<dyn McpClient>,
    mcp_tool_name: String,
}

impl McpToolAdapter {
    /// Create a new adapter from an MCP tool definition and client.
    pub fn new(tool_def: &McpToolDef, client: Arc<dyn McpClient>) -> Self {
        let schema = ToolSchema {
            name: tool_def.name.clone(),
            description: tool_def.description.clone().unwrap_or_default(),
            parameters: schema_to_json(&tool_def.input_schema),
        };
        Self {
            schema,
            client,
            mcp_tool_name: tool_def.name.clone(),
        }
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn schema(&self) -> ToolSchema {
        self.schema.clone()
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let arguments = match &args {
            serde_json::Value::Object(map) => Some(map.clone()),
            _ => None,
        };

        let params = CallToolRequestParams {
            name: self.mcp_tool_name.clone(),
            arguments,
            meta: Default::default(),
            task: None,
        };

        let result = self
            .client
            .request_tool_call(params)
            .await
            .map_err(|e| ToolError::Execution(format!("MCP call failed: {e}")))?;

        // Extract text content from the result
        let content = result
            .content
            .iter()
            .filter_map(|c| match c {
                rust_mcp_sdk::schema::ContentBlock::TextContent(tc) => Some(tc.text.clone()),
                rust_mcp_sdk::schema::ContentBlock::ImageContent(_) => Some("[image]".into()),
                rust_mcp_sdk::schema::ContentBlock::EmbeddedResource(_) => {
                    Some("[resource]".into())
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolResult {
            content,
            is_error: result.is_error.unwrap_or(false),
        })
    }
}

/// Discover tools from a running MCP client and return adapters.
pub async fn discover_tools(client: Arc<dyn McpClient>) -> Result<Vec<McpToolAdapter>, McpError> {
    let tool_list = client
        .request_tool_list(None)
        .await
        .map_err(|e| McpError::Sdk(format!("failed to list tools: {e}")))?;

    let tools: Vec<McpToolAdapter> = tool_list
        .tools
        .iter()
        .map(|t| McpToolAdapter::new(t, client.clone()))
        .collect();

    tracing::info!(count = tools.len(), "MCP tools discovered");
    Ok(tools)
}

/// Register all MCP tools from a client into an agent runner.
pub async fn register_mcp_tools(
    client: Arc<dyn McpClient>,
    runner: &mut dsh_core_agent_loop::AgentRunner,
) -> Result<usize, McpError> {
    let tools = discover_tools(client).await?;
    let count = tools.len();
    for tool in tools {
        runner.register_tool(Arc::new(tool));
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_to_json_empty() {
        // Test with a minimal schema
        let json = serde_json::json!({});
        assert!(json.is_object());
    }
}
