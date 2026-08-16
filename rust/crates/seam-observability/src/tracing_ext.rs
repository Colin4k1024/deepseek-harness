//! Distributed tracing with OpenTelemetry integration.
//!
//! When the `otel` feature is enabled, provides OpenTelemetry tracing
//! that exports to OTLP-compatible backends (Jaeger, Grafana Tempo, etc.).

use tracing_subscriber::prelude::*;

/// Distributed tracing configuration.
#[derive(Debug, Clone)]
pub struct TracingConfig {
    /// Service name for traces.
    pub service_name: String,

    /// OTLP endpoint (e.g., "http://localhost:4317").
    pub otlp_endpoint: Option<String>,

    /// Sampling ratio (0.0 - 1.0).
    pub sample_ratio: f64,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            service_name: "dsh".into(),
            otlp_endpoint: None,
            sample_ratio: 1.0,
        }
    }
}

/// Initialize distributed tracing.
///
/// When `otel` feature is enabled and endpoint is provided, exports traces via OTLP.
/// Otherwise, falls back to local tracing only.
pub fn init_tracing(config: &TracingConfig) -> Result<(), TracingError> {
    #[cfg(feature = "otel")]
    {
        if let Some(endpoint) = &config.otlp_endpoint {
            return init_otel_tracing(config.service_name.clone(), endpoint.clone());
        }
    }

    tracing::info!(
        service = %config.service_name,
        "distributed tracing not configured (no OTLP endpoint)"
    );
    Ok(())
}

#[cfg(feature = "otel")]
fn init_otel_tracing(service_name: String, endpoint: String) -> Result<(), TracingError> {
    use opentelemetry::trace::TracerProvider;
    use opentelemetry_otlp::WithExportConfig;

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&endpoint)
        .build()
        .map_err(|e| TracingError::Init(format!("OTLP exporter error: {e}")))?;

    let provider = opentelemetry_sdk::trace::TracerProvider::builder()
        .with_simple_exporter(exporter)
        .build();

    let tracer = provider.tracer(service_name.clone());

    opentelemetry::global::set_tracer_provider(provider);

    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry().with(otel_layer).init();

    tracing::info!(
        service = %service_name,
        endpoint = %endpoint,
        "OpenTelemetry tracing initialized"
    );

    Ok(())
}

/// Tracing errors.
#[derive(Debug, thiserror::Error)]
pub enum TracingError {
    #[error("tracing initialization error: {0}")]
    Init(String),
}

/// Create a traced span for an agent operation.
///
/// Usage:
/// ```rust
/// use dsh_seam_observability::tracing_ext::agent_span;
///
/// let _span = agent_span("chat", "user-123").entered();
/// // ... do work ...
/// ```
pub fn agent_span(operation: &str, user_id: &str) -> tracing::Span {
    tracing::info_span!(
        "agent_operation",
        operation = %operation,
        user_id = %user_id,
    )
}

/// Create a traced span for an LLM call.
pub fn llm_span(model: &str, tokens: u32) -> tracing::Span {
    tracing::info_span!(
        "llm_call",
        model = %model,
        tokens = %tokens,
    )
}

/// Create a traced span for a tool call.
pub fn tool_span(tool_name: &str) -> tracing::Span {
    tracing::info_span!(
        "tool_call",
        tool_name = %tool_name,
    )
}

/// Create a traced span for a workflow step.
pub fn workflow_span(workflow_id: &str, step_id: &str) -> tracing::Span {
    tracing::info_span!(
        "workflow_step",
        workflow_id = %workflow_id,
        step_id = %step_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tracing_config() {
        let config = TracingConfig::default();
        assert_eq!(config.service_name, "dsh");
        assert!(config.otlp_endpoint.is_none());
        assert_eq!(config.sample_ratio, 1.0);
    }

    #[test]
    fn agent_span_creation() {
        // Just verify the span can be created without panicking
        let _span = agent_span("chat", "user-123");
    }
}
