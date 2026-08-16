//! Multi-agent orchestration — agent registry, message bus, and task delegation.
//!
//! Enables multiple agents to discover each other, communicate, and delegate
//! subtasks. Supports fan-out (parallel), fan-in (collect), and sequential
//! orchestration patterns.

pub mod agent;
pub mod message;
pub mod orchestrator;
pub mod registry;

pub use agent::AgentHandle;
pub use message::{AgentMessage, MessageType};
pub use orchestrator::Orchestrator;
pub use registry::AgentRegistry;
