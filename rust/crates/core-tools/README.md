# dsh-core-tools

L1: Tool registry and execution pipeline.

## Purpose

Defines the `Tool` trait for agent-invocable tools, plus a `ToolPipeline` with pre/post middleware chain semantics.

## Key Types

- **`Tool`** — Async trait: `schema()` returns metadata, `execute(args)` runs the tool.
- **`ToolSchema`** — Name, description, and JSON Schema for parameters.
- **`ToolResult`** — Content string + `is_error` flag.
- **`ToolPipeline`** — Pre-hooks (can reject) + post-hooks (can modify result).

## Usage

```rust
use dsh_core_tools::{Tool, ToolSchema, ToolResult, ToolError};
use async_trait::async_trait;

struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema { name: "echo".into(), description: "Echo input".into(), parameters: serde_json::json!({}) }
    }
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        Ok(ToolResult { content: args.to_string(), is_error: false })
    }
}
```
