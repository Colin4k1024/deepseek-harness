//! Span builder — creates and manages trace spans.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

use super::context::{SpanId, TraceContext, TraceId};

/// Span status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpanStatus {
    Ok,
    Error { message: String },
    Unset,
}

/// Span event (timestamped annotation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    pub name: String,
    pub timestamp: String,
    pub attributes: HashMap<String, String>,
}

/// A completed span with all metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
    pub name: String,
    pub start_time: String,
    pub duration_ms: u64,
    pub status: SpanStatus,
    pub events: Vec<SpanEvent>,
    pub attributes: HashMap<String, String>,
}

/// Span builder — creates spans with fluent API.
pub struct SpanBuilder {
    context: TraceContext,
    name: String,
    attributes: HashMap<String, String>,
    start: Instant,
}

impl SpanBuilder {
    /// Create a new span builder.
    pub fn new(name: impl Into<String>, context: TraceContext) -> Self {
        Self {
            context,
            name: name.into(),
            attributes: HashMap::new(),
            start: Instant::now(),
        }
    }

    /// Add an attribute.
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Finish the span and return it.
    pub fn finish(self, status: SpanStatus) -> Span {
        Span {
            trace_id: self.context.trace_id,
            span_id: self.context.span_id,
            parent_span_id: self.context.parent_span_id,
            name: self.name,
            start_time: now_iso(),
            duration_ms: self.start.elapsed().as_millis() as u64,
            status,
            events: Vec::new(),
            attributes: self.attributes,
        }
    }

    /// Get the trace context for propagation.
    pub fn context(&self) -> &TraceContext {
        &self.context
    }
}

fn now_iso() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_span() {
        let ctx = TraceContext::new_root();
        let span = SpanBuilder::new("test_span", ctx)
            .with_attribute("key", "value")
            .finish(SpanStatus::Ok);

        assert_eq!(span.name, "test_span");
        assert_eq!(span.attributes.get("key").unwrap(), "value");
        assert!(matches!(span.status, SpanStatus::Ok));
    }

    #[test]
    fn span_with_error() {
        let ctx = TraceContext::new_root();
        let span = SpanBuilder::new("error_span", ctx).finish(SpanStatus::Error {
            message: "something failed".into(),
        });

        assert!(matches!(span.status, SpanStatus::Error { .. }));
    }
}
