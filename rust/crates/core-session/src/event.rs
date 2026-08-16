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

// ── Message surface types ────────────────────────────────────────────

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

/// How a surface event entered the ordered surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op")]
pub enum SurfaceOp {
    /// Appended to the tail.
    #[serde(rename = "append")]
    Append,
    /// Replaces surface nodes from `start` (inclusive) through `end` (inclusive).
    #[serde(rename = "replace")]
    Replace { start: u64, end: u64 },
}

/// Trait for projecting session events into model-visible messages.
pub trait Surface {
    fn project(&self) -> Option<ModelMessage>;
    fn surface_op(&self) -> Option<SurfaceOp>;
    fn source_event_seqs(&self) -> Option<&[u64]>;
}

// ── Turn / step lifecycle types ──────────────────────────────────────

/// Why a turn ended.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum TurnEndReason {
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "aborted")]
    Aborted { reason: String },
    #[serde(rename = "blocked")]
    Blocked,
    #[serde(rename = "error")]
    Error { error: LlmFailure },
    #[serde(rename = "max-tokens")]
    MaxTokens,
    #[serde(rename = "interrupted")]
    Interrupted,
}

/// Structured LLM failure payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmFailure {
    pub message: String,
    pub code: String,
}

/// Token usage reported by the model adapter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Call configuration for one request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmCallConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

/// Tool schema definition for request headers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolSchemaDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Snapshot of the request header for one epoch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EpochHeader {
    pub config: LlmCallConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolSchemaDef>>,
}

/// Why a request header was appended.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RequestHeaderReason {
    Initial,
    Resume,
    Change,
}

// ── Session event ────────────────────────────────────────────────────

/// One immutable entry in the session log.
///
/// Tagged enum with `seq` (monotonic) and `time` (Unix epoch ms) on every variant.
/// Lifecycle events (`turn/start`, `turn/end`, `step/start`, `step/end`) carry
/// no model-visible surface data; message events project via the `Surface` trait.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum SessionEvent {
    /// Opens a turn before the loop claims queued input.
    #[serde(rename = "turn/start")]
    TurnStart {
        seq: u64,
        time: u64,
        turn: u64,
    },
    /// Closes a turn with the reason that ended it.
    #[serde(rename = "turn/end")]
    TurnEnd {
        seq: u64,
        time: u64,
        turn: u64,
        reason: TurnEndReason,
    },
    /// Opens one step inside a turn.
    #[serde(rename = "step/start")]
    StepStart {
        seq: u64,
        time: u64,
        turn: u64,
        step: u64,
    },
    /// Closes one step.
    #[serde(rename = "step/end")]
    StepEnd {
        seq: u64,
        time: u64,
        turn: u64,
        step: u64,
    },
    /// A user-role message on the model-visible surface.
    #[serde(rename = "user/message")]
    UserMessage {
        seq: u64,
        time: u64,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
    /// Raw stream chunk — token-level replay fidelity.
    #[serde(rename = "assistant/chunk")]
    AssistantChunk {
        seq: u64,
        time: u64,
        turn: u64,
        step: u64,
        content: String,
    },
    /// Assembled assistant message for one step.
    #[serde(rename = "assistant/message")]
    AssistantMessage {
        seq: u64,
        time: u64,
        turn: u64,
        step: u64,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<TokenUsage>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_event_seqs: Option<Vec<u64>>,
    },
    /// The model requested one tool invocation.
    #[serde(rename = "tool/call")]
    ToolCall {
        seq: u64,
        time: u64,
        turn: u64,
        step: u64,
        call_id: String,
        name: String,
        arguments: String,
    },
    /// A completed tool call's result.
    #[serde(rename = "tool/result")]
    ToolResult {
        seq: u64,
        time: u64,
        turn: u64,
        step: u64,
        call_id: String,
        content: String,
        is_error: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_event_seqs: Option<Vec<u64>>,
    },
    /// Full header for the next request, appended inside its step before dispatch.
    #[serde(rename = "request/header")]
    RequestHeader {
        seq: u64,
        time: u64,
        header: EpochHeader,
        reason: RequestHeaderReason,
    },
    /// Marks the end of a constructor seed.
    #[serde(rename = "session/end-seed")]
    EndSeed {
        seq: u64,
        time: u64,
    },
}

impl SessionEvent {
    /// Return the monotonic sequence number.
    pub fn seq(&self) -> u64 {
        match self {
            Self::TurnStart { seq, .. } => *seq,
            Self::TurnEnd { seq, .. } => *seq,
            Self::StepStart { seq, .. } => *seq,
            Self::StepEnd { seq, .. } => *seq,
            Self::UserMessage { seq, .. } => *seq,
            Self::AssistantChunk { seq, .. } => *seq,
            Self::AssistantMessage { seq, .. } => *seq,
            Self::ToolCall { seq, .. } => *seq,
            Self::ToolResult { seq, .. } => *seq,
            Self::RequestHeader { seq, .. } => *seq,
            Self::EndSeed { seq, .. } => *seq,
        }
    }

    /// Return the Unix epoch milliseconds timestamp.
    pub fn time(&self) -> u64 {
        match self {
            Self::TurnStart { time, .. } => *time,
            Self::TurnEnd { time, .. } => *time,
            Self::StepStart { time, .. } => *time,
            Self::StepEnd { time, .. } => *time,
            Self::UserMessage { time, .. } => *time,
            Self::AssistantChunk { time, .. } => *time,
            Self::AssistantMessage { time, .. } => *time,
            Self::ToolCall { time, .. } => *time,
            Self::ToolResult { time, .. } => *time,
            Self::RequestHeader { time, .. } => *time,
            Self::EndSeed { time, .. } => *time,
        }
    }

    /// Whether this event is ignorable by readers that don't recognize its type.
    pub fn ignorable(&self) -> bool {
        matches!(self, Self::AssistantChunk { .. } | Self::EndSeed { .. })
    }
}

impl Surface for SessionEvent {
    fn project(&self) -> Option<ModelMessage> {
        match self {
            Self::UserMessage { content, .. } => Some(ModelMessage {
                role: Role::User,
                content: content.clone(),
            }),
            Self::AssistantMessage { message, .. } => Some(ModelMessage {
                role: Role::Assistant,
                content: message.clone(),
            }),
            Self::ToolCall {
                call_id, name, arguments, ..
            } => Some(ModelMessage {
                role: Role::Assistant,
                content: serde_json::json!({
                    "tool_call": { "id": call_id, "name": name, "arguments": arguments }
                })
                .to_string(),
            }),
            Self::ToolResult {
                call_id, content, ..
            } => Some(ModelMessage {
                role: Role::Tool,
                content: format!("[{call_id}] {content}"),
            }),
            // Lifecycle events and chunks do not project.
            Self::TurnStart { .. }
            | Self::TurnEnd { .. }
            | Self::StepStart { .. }
            | Self::StepEnd { .. }
            | Self::AssistantChunk { .. }
            | Self::RequestHeader { .. }
            | Self::EndSeed { .. } => None,
        }
    }

    fn surface_op(&self) -> Option<SurfaceOp> {
        match self {
            Self::UserMessage { .. }
            | Self::AssistantMessage { .. }
            | Self::ToolResult { .. } => Some(SurfaceOp::Append),
            _ => None,
        }
    }

    fn source_event_seqs(&self) -> Option<&[u64]> {
        match self {
            Self::AssistantMessage {
                source_event_seqs, ..
            } => source_event_seqs.as_deref(),
            Self::ToolResult {
                source_event_seqs, ..
            } => source_event_seqs.as_deref(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_roundtrip_user_message() {
        let event = SessionEvent::UserMessage {
            seq: 1,
            time: 1700000000000,
            content: "hello".into(),
            turn: Some(1),
            source: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: SessionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.seq(), 1);
        assert_eq!(parsed.time(), 1700000000000);
        assert!(matches!(parsed, SessionEvent::UserMessage { .. }));
    }

    #[test]
    fn serde_roundtrip_turn_lifecycle() {
        let events = vec![
            SessionEvent::TurnStart { seq: 1, time: 1000, turn: 1 },
            SessionEvent::StepStart { seq: 2, time: 1001, turn: 1, step: 1 },
            SessionEvent::StepEnd { seq: 3, time: 1002, turn: 1, step: 1 },
            SessionEvent::TurnEnd { seq: 4, time: 1003, turn: 1, reason: TurnEndReason::Completed },
        ];
        let json = serde_json::to_string(&events).unwrap();
        let parsed: Vec<SessionEvent> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 4);
    }

    #[test]
    fn surface_projection() {
        let user = SessionEvent::UserMessage {
            seq: 1, time: 1000, content: "hi".into(), turn: Some(1), source: None,
        };
        let msg = user.project().unwrap();
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "hi");

        let chunk = SessionEvent::AssistantChunk {
            seq: 2, time: 1001, turn: 1, step: 1, content: "partial".into(),
        };
        assert!(chunk.project().is_none());
    }

    #[test]
    fn ignorable_events() {
        assert!(SessionEvent::AssistantChunk {
            seq: 1, time: 1000, turn: 1, step: 1, content: "x".into(),
        }
        .ignorable());
        assert!(SessionEvent::EndSeed { seq: 1, time: 1000 }.ignorable());
        assert!(!SessionEvent::UserMessage {
            seq: 1, time: 1000, content: "x".into(), turn: None, source: None,
        }
        .ignorable());
    }
}