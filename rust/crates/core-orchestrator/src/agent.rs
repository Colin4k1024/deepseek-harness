//! AgentHandle — represents a running agent in the orchestration system.

use tokio::sync::mpsc;

use crate::message::AgentMessage;

/// Handle to a running agent.
///
/// Contains the agent's identity, capabilities, and a channel for sending messages.
#[derive(Debug, Clone)]
pub struct AgentHandle {
    /// Unique agent ID.
    pub id: String,

    /// Agent name (human-readable).
    pub name: String,

    /// Agent capabilities (e.g., "code-review", "research", "writing").
    pub capabilities: Vec<String>,

    /// Channel sender for delivering messages to this agent.
    tx: mpsc::UnboundedSender<AgentMessage>,
}

impl AgentHandle {
    /// Create a new agent handle.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        capabilities: Vec<String>,
    ) -> (Self, mpsc::UnboundedReceiver<AgentMessage>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let handle = Self {
            id: id.into(),
            name: name.into(),
            capabilities,
            tx,
        };
        (handle, rx)
    }

    /// Send a message to this agent.
    pub fn send(&self, message: AgentMessage) -> Result<(), mpsc::error::SendError<AgentMessage>> {
        self.tx.send(message)
    }

    /// Check if this agent has a specific capability.
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.iter().any(|c| c == cap)
    }
}

/// Status of an agent.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    /// Agent is idle and ready to accept tasks.
    Idle,
    /// Agent is currently working on a task.
    Busy { task_id: String },
    /// Agent has stopped.
    Stopped,
}
