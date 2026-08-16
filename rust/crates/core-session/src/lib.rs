//! L1: Session log — the source of truth for all model-visible state.
//!
//! Defines `SessionEvent` (tagged enum), the `Surface` trait (projection into
//! model-visible messages), and the session log invariant assertion.

pub mod derive;
pub mod event;
pub mod persist;

pub use event::{
    EpochHeader, LlmCallConfig, LlmFailure, ModelMessage, RequestHeaderReason, Role, SessionError,
    SessionEvent, Surface, SurfaceOp, TokenUsage, ToolSchemaDef, TurnEndReason,
};
pub use persist::SessionHeader;