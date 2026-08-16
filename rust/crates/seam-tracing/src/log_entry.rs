//! Log entry — structured log record.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace => write!(f, "TRACE"),
            Self::Debug => write!(f, "DEBUG"),
            Self::Info => write!(f, "INFO"),
            Self::Warn => write!(f, "WARN"),
            Self::Error => write!(f, "ERROR"),
        }
    }
}

/// Structured log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp (Unix seconds).
    pub timestamp: u64,

    /// Log level.
    pub level: LogLevel,

    /// Log message.
    pub message: String,

    /// Source module/file.
    pub source: Option<String>,

    /// Trace ID for correlation.
    pub trace_id: Option<String>,

    /// Span ID for correlation.
    pub span_id: Option<String>,

    /// Additional structured fields.
    pub fields: HashMap<String, serde_json::Value>,
}

impl LogEntry {
    /// Create a new log entry.
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            timestamp: now(),
            level,
            message: message.into(),
            source: None,
            trace_id: None,
            span_id: None,
            fields: HashMap::new(),
        }
    }

    /// Set the source.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set trace context.
    pub fn with_trace(mut self, trace_id: impl Into<String>, span_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self.span_id = Some(span_id.into());
        self
    }

    /// Add a field.
    pub fn with_field(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.fields.insert(key.into(), value);
        self
    }

    /// Format as a single-line text string.
    pub fn to_text(&self) -> String {
        let mut parts = vec![format!("[{}]", self.level)];

        if let Some(trace_id) = &self.trace_id {
            parts.push(format!("[trace:{trace_id}]"));
        }

        parts.push(self.message.clone());

        if !self.fields.is_empty() {
            let fields: Vec<String> = self
                .fields
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            parts.push(format!("{{{}}}", fields.join(", ")));
        }

        parts.join(" ")
    }

    /// Format as JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_entry() {
        let entry = LogEntry::new(LogLevel::Info, "test message");
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.message, "test message");
    }

    #[test]
    fn with_fields() {
        let entry = LogEntry::new(LogLevel::Error, "failed")
            .with_source("auth.rs")
            .with_trace("abc123", "span1")
            .with_field("user_id", serde_json::json!("user-1"));

        assert_eq!(entry.source, Some("auth.rs".into()));
        assert_eq!(entry.trace_id, Some("abc123".into()));
        assert_eq!(entry.fields.len(), 1);
    }

    #[test]
    fn to_text() {
        let entry = LogEntry::new(LogLevel::Info, "hello");
        let text = entry.to_text();
        assert!(text.contains("[INFO]"));
        assert!(text.contains("hello"));
    }

    #[test]
    fn to_json() {
        let entry = LogEntry::new(LogLevel::Info, "hello");
        let json = entry.to_json();
        assert!(json.contains("\"level\":\"info\""));
        assert!(json.contains("\"message\":\"hello\""));
    }

    #[test]
    fn log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }
}
