//! Trace propagation — inject/extract trace context from HTTP headers.

use std::collections::HashMap;

use super::context::TraceContext;

/// Propagation errors.
#[derive(Debug, thiserror::Error)]
pub enum PropagationError {
    #[error("missing header: {0}")]
    MissingHeader(String),
    #[error("invalid header format: {0}")]
    InvalidFormat(String),
}

/// Trace propagator — injects/extracts trace context from headers.
pub struct TracePropagator;

impl TracePropagator {
    /// Extract trace context from HTTP headers.
    pub fn extract(headers: &HashMap<String, String>) -> Result<TraceContext, PropagationError> {
        let traceparent = headers
            .get("traceparent")
            .ok_or_else(|| PropagationError::MissingHeader("traceparent".into()))?;

        TraceContext::from_traceparent(traceparent)
            .ok_or_else(|| PropagationError::InvalidFormat(traceparent.clone()))
    }

    /// Inject trace context into HTTP headers.
    pub fn inject(context: &TraceContext, headers: &mut HashMap<String, String>) {
        headers.insert("traceparent".into(), context.to_traceparent());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_and_inject() {
        let ctx = TraceContext::new_root();
        let mut headers = HashMap::new();
        TracePropagator::inject(&ctx, &mut headers);

        let extracted = TracePropagator::extract(&headers).unwrap();
        assert_eq!(extracted.trace_id, ctx.trace_id);
        assert_eq!(extracted.span_id, ctx.span_id);
    }

    #[test]
    fn extract_missing_header() {
        let headers = HashMap::new();
        assert!(TracePropagator::extract(&headers).is_err());
    }

    #[test]
    fn extract_invalid_format() {
        let mut headers = HashMap::new();
        headers.insert("traceparent".into(), "invalid".into());
        assert!(TracePropagator::extract(&headers).is_err());
    }
}
