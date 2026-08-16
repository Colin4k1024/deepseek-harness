//! Message — translatable message identifiers and content.

use serde::{Deserialize, Serialize};

/// Pre-defined message IDs for common messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageId {
    // Errors
    InvalidApiKey,
    RateLimitExceeded,
    QuotaExceeded,
    SessionNotFound,
    ToolNotFound,
    LlmError,
    SandboxUnavailable,
    InternalError,

    // Agent
    AgentStarted,
    AgentCompleted,
    AgentFailed,

    // Tool
    ToolExecuted,
    ToolFailed,

    // Session
    SessionCreated,
    SessionClosed,

    // General
    Welcome,
    Goodbye,
    Loading,
    Success,
    Failure,
}

/// A translatable message with parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Message ID.
    pub id: MessageId,

    /// Parameters for interpolation.
    pub params: std::collections::HashMap<String, String>,
}

impl Message {
    /// Create a new message with no parameters.
    pub fn new(id: MessageId) -> Self {
        Self {
            id,
            params: std::collections::HashMap::new(),
        }
    }

    /// Add a parameter.
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }
}
