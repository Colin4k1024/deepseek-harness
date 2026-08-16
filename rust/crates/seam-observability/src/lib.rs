//! L2: Observability seam — tracing, metrics, and structured logging.
//!
//! Provides a unified telemetry layer for the dsh agent framework.
//! Uses the `tracing` crate for structured logging and custom metrics
//! collection for agent operations.
//!
//! Optional OpenTelemetry integration via the `otel` feature flag.

pub mod metrics;
pub mod telemetry;
pub mod tracing_ext;

pub use metrics::Metrics;
pub use telemetry::{Telemetry, TelemetryConfig};
