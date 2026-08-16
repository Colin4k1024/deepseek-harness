use thiserror::Error;

/// Errors from the agent loop.
#[derive(Debug, Error)]
pub enum LoopError {
    #[error("llm error: {0}")]
    Llm(#[from] dsh_llm_adapter::LlmError),
    #[error("tool error: {0}")]
    Tool(#[from] dsh_core_tools::ToolError),
    #[error("tool not found: {0}")]
    ToolNotFound(String),
    #[error("loop closed")]
    Closed,
}

/// States of the agent loop turn state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnState {
    /// Waiting for user input.
    ClaimingInput,
    /// Assembling the prompt from session log.
    AssemblingPrompt,
    /// Streaming LLM response.
    Streaming,
    /// Executing tool calls.
    ExecutingTools { tool_name: String },
    /// Turn is closed — assistant produced a final answer.
    Closed,
}
