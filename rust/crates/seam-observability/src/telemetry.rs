//! Telemetry initialization — tracing subscriber setup.
//!
//! Configures structured logging with optional OpenTelemetry export.
//! Supports two modes:
//! - **Development**: pretty-printed stdout logging
//! - **Production**: JSON-formatted logging (for log aggregators)

use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

/// Telemetry configuration.
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Log level filter (e.g., "info", "debug", "dsh=debug").
    pub log_level: String,

    /// Output format.
    pub format: LogFormat,

    /// Whether to include file/line info.
    pub include_location: bool,
}

/// Log output format.
#[derive(Debug, Clone, PartialEq)]
pub enum LogFormat {
    /// Pretty-printed for development.
    Pretty,
    /// JSON for production log aggregators.
    Json,
    /// Compact single-line.
    Compact,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            log_level: "info".into(),
            format: LogFormat::Pretty,
            include_location: true,
        }
    }
}

/// Telemetry handle — initializes and manages the tracing subscriber.
pub struct Telemetry {
    config: TelemetryConfig,
}

impl Telemetry {
    /// Create a new telemetry instance with default config.
    pub fn new() -> Self {
        Self {
            config: TelemetryConfig::default(),
        }
    }

    /// Create with custom config.
    pub fn with_config(config: TelemetryConfig) -> Self {
        Self { config }
    }

    /// Initialize the tracing subscriber.
    ///
    /// Should be called once at application startup, before any logging.
    pub fn init(&self) {
        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&self.config.log_level));

        match self.config.format {
            LogFormat::Pretty => {
                let fmt_layer = fmt::layer()
                    .pretty()
                    .with_file(self.config.include_location)
                    .with_line_number(self.config.include_location);
                tracing_subscriber::registry()
                    .with(filter)
                    .with(fmt_layer)
                    .init();
            }
            LogFormat::Json => {
                let fmt_layer = fmt::layer()
                    .json()
                    .with_file(self.config.include_location)
                    .with_line_number(self.config.include_location);
                tracing_subscriber::registry()
                    .with(filter)
                    .with(fmt_layer)
                    .init();
            }
            LogFormat::Compact => {
                let fmt_layer = fmt::layer()
                    .compact()
                    .with_file(self.config.include_location)
                    .with_line_number(self.config.include_location);
                tracing_subscriber::registry()
                    .with(filter)
                    .with(fmt_layer)
                    .init();
            }
        }
    }

    /// Create a development telemetry instance (pretty logging, debug level).
    pub fn development() -> Self {
        Self::with_config(TelemetryConfig {
            log_level: "debug".into(),
            format: LogFormat::Pretty,
            include_location: true,
        })
    }

    /// Create a production telemetry instance (JSON logging, info level).
    pub fn production() -> Self {
        Self::with_config(TelemetryConfig {
            log_level: "info".into(),
            format: LogFormat::Json,
            include_location: false,
        })
    }
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}
