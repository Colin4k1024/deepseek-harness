//! Trace context — trace ID and span ID management.

use serde::{Deserialize, Serialize};

/// W3C Trace Context trace ID (128-bit, 32 hex chars).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId(pub String);

impl TraceId {
    /// Generate a new random trace ID.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string().replace('-', ""))
    }

    /// Parse from a hex string.
    pub fn from_hex(hex: &str) -> Option<Self> {
        if hex.len() == 32 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            Some(Self(hex.to_string()))
        } else {
            None
        }
    }

    /// Get as hex string.
    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

/// W3C Trace Context span ID (64-bit, 16 hex chars).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpanId(pub String);

impl SpanId {
    /// Generate a new random span ID.
    pub fn new() -> Self {
        let uuid = uuid::Uuid::new_v4().to_string().replace('-', "");
        Self(uuid[..16].to_string())
    }

    /// Get as hex string.
    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl Default for SpanId {
    fn default() -> Self {
        Self::new()
    }
}

/// Trace context — propagated across service boundaries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    /// Trace ID.
    pub trace_id: TraceId,

    /// Parent span ID.
    pub parent_span_id: Option<SpanId>,

    /// Current span ID.
    pub span_id: SpanId,

    /// Trace flags (e.g., sampled).
    pub flags: u8,
}

impl TraceContext {
    /// Create a new root trace context.
    pub fn new_root() -> Self {
        Self {
            trace_id: TraceId::new(),
            parent_span_id: None,
            span_id: SpanId::new(),
            flags: 1, // sampled
        }
    }

    /// Create a child span context.
    pub fn child(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            parent_span_id: Some(self.span_id.clone()),
            span_id: SpanId::new(),
            flags: self.flags,
        }
    }

    /// Parse from W3C traceparent header format: `{trace-id}-{parent-span-id}-{flags}`.
    pub fn from_traceparent(header: &str) -> Option<Self> {
        let parts: Vec<&str> = header.split('-').collect();
        if parts.len() != 4 {
            return None;
        }

        let trace_id = TraceId::from_hex(parts[1])?;
        let span_id = SpanId(parts[2].to_string());
        let flags = u8::from_str_radix(parts[3], 16).ok()?;

        Some(Self {
            trace_id,
            parent_span_id: None,
            span_id,
            flags,
        })
    }

    /// Serialize to W3C traceparent header format.
    pub fn to_traceparent(&self) -> String {
        format!(
            "00-{}-{}-{:02x}",
            self.trace_id.as_hex(),
            self.span_id.as_hex(),
            self.flags
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_id_generation() {
        let id = TraceId::new();
        assert_eq!(id.as_hex().len(), 32);
    }

    #[test]
    fn span_id_generation() {
        let id = SpanId::new();
        assert_eq!(id.as_hex().len(), 16);
    }

    #[test]
    fn root_context() {
        let ctx = TraceContext::new_root();
        assert!(ctx.parent_span_id.is_none());
        assert_eq!(ctx.flags, 1);
    }

    #[test]
    fn child_context() {
        let root = TraceContext::new_root();
        let child = root.child();
        assert_eq!(child.trace_id, root.trace_id);
        assert_eq!(child.parent_span_id.as_ref().unwrap(), &root.span_id);
        assert_ne!(child.span_id, root.span_id);
    }

    #[test]
    fn traceparent_roundtrip() {
        let ctx = TraceContext::new_root();
        let header = ctx.to_traceparent();
        let parsed = TraceContext::from_traceparent(&header).unwrap();
        assert_eq!(parsed.trace_id, ctx.trace_id);
        assert_eq!(parsed.span_id, ctx.span_id);
        assert_eq!(parsed.flags, ctx.flags);
    }

    #[test]
    fn traceparent_invalid() {
        assert!(TraceContext::from_traceparent("invalid").is_none());
        assert!(TraceContext::from_traceparent("00-abc-123-01").is_none()); // wrong lengths
    }
}
