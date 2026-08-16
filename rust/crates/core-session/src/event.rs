use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors from session operations.
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// A message visible to the LLM model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelMessage {
    pub role: Role,
    pub content: String,
}

/// Message role.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// Trait for projecting session events into model-visible messages.
pub trait Surface {
    /// Project this event into a model-visible message, or `None` if not model-visible.
    fn project(&self) -> Option<ModelMessage>;
}

/// Closed enum of all session events.
///
/// New model-visible event types MUST be added here — the compiler will
/// force all match sites to handle the new variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SessionEvent {
    UserMessage {
        content: String,
    },
    AssistantMessage {
        content: String,
    },
    AssistantChunk {
        content: String,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    ToolResult {
        call_id: String,
        content: String,
    },
}

impl Surface for SessionEvent {
    fn project(&self) -> Option<ModelMessage> {
        match self {
            Self::UserMessage { content } => Some(ModelMessage {
                role: Role::User,
                content: content.clone(),
            }),
            Self::AssistantMessage { content } => Some(ModelMessage {
                role: Role::Assistant,
                content: content.clone(),
            }),
            Self::AssistantChunk { .. } => None, // Chunks aggregate into AssistantMessage
            Self::ToolCall {
                id,
                name,
                arguments,
            } => Some(ModelMessage {
                role: Role::Assistant,
                content: serde_json::json!({
                    "tool_call": { "id": id, "name": name, "arguments": arguments }
                })
                .to_string(),
            }),
            Self::ToolResult { call_id, content } => Some(ModelMessage {
                role: Role::Tool,
                content: format!("[{call_id}] {content}"),
            }),
        }
    }
}
