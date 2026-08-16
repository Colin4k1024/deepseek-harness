//! L1: Agent loop — turn/step state machine for LLM-tool interaction.
//!
//! Drives the `ClaimingInput → AssemblingPrompt → Streaming → ExecutingTools → Closed`
//! state machine, consuming `LlmAdapter` and `ToolPipeline`.

pub mod hooks;
pub mod runner;
pub mod state;

pub use hooks::AgentHooks;
pub use runner::AgentRunner;
pub use state::{LoopError, TurnState};
