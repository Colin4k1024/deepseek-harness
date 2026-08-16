//! Message types for agent-to-agent communication.

use serde::{Deserialize, Serialize};

/// A message sent between agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Unique message ID.
    pub id: String,

    /// Sender agent ID.
    pub from: String,

    /// Recipient agent ID (or "*" for broadcast).
    pub to: String,

    /// Message type.
    pub message_type: MessageType,

    /// Message payload (arbitrary JSON).
    pub payload: serde_json::Value,

    /// Timestamp (Unix seconds).
    pub timestamp: u64,
}

/// Type of agent message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    /// Request: ask another agent to perform a task.
    TaskRequest,

    /// Response: result of a task request.
    TaskResponse,

    /// Informational: share context or findings.
    Info,

    /// Query: ask a question.
    Query,

    /// Heartbeat: check if agent is alive.
    Heartbeat,

    /// Shutdown: tell agent to stop.
    Shutdown,
}

impl AgentMessage {
    /// Create a new task request message.
    pub fn task_request(from: &str, to: &str, task: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            from: from.into(),
            to: to.into(),
            message_type: MessageType::TaskRequest,
            payload: serde_json::json!({ "task": task }),
            timestamp: now(),
        }
    }

    /// Create a task response message.
    pub fn task_response(from: &str, to: &str, result: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            from: from.into(),
            to: to.into(),
            message_type: MessageType::TaskResponse,
            payload: serde_json::json!({ "result": result }),
            timestamp: now(),
        }
    }

    /// Create an info message.
    pub fn info(from: &str, to: &str, content: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            from: from.into(),
            to: to.into(),
            message_type: MessageType::Info,
            payload: serde_json::json!({ "content": content }),
            timestamp: now(),
        }
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
