# dsh-core-agent-loop

L1: Agent loop — turn/step state machine for LLM-tool interaction.

## Purpose

Drives the agent conversation loop: user input → LLM call → tool execution → final answer. This is the core orchestration layer.

## Key Types

- **`AgentRunner`** — Owns the LLM adapter, tool registry, and session log. `run_turn(message)` drives the full state machine.
- **`TurnState`** — Enum: `ClaimingInput → AssemblingPrompt → Streaming → ExecutingTools → Closed`.

## State Machine

```
UserMessage added to session log
  → derive_messages() → LlmRequest
  → LLM stream → AssistantChunk events
  → parse for tool_call JSON
    ├─ tool call → execute → ToolCall + ToolResult → loop
    └─ no tool call → AssistantMessage → Closed → return
```

## Usage

```rust
use dsh_core_agent_loop::AgentRunner;
use std::sync::Arc;

let llm = Arc::new(my_llm_adapter);
let mut runner = AgentRunner::new(llm, "deepseek-chat".into());
runner.register_tool(Arc::new(my_tool));
let response = runner.run_turn("What is 2+2?".into()).await?;
```
