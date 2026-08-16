//! Log aggregator — collects and forwards structured logs.

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::log_entry::{LogEntry, LogLevel};

/// Aggregator configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatorConfig {
    /// Minimum log level to collect.
    pub min_level: LogLevel,

    /// Buffer size before flushing.
    pub buffer_size: usize,

    /// Flush interval in seconds.
    pub flush_interval_secs: u64,
}

impl Default for AggregatorConfig {
    fn default() -> Self {
        Self {
            min_level: LogLevel::Info,
            buffer_size: 100,
            flush_interval_secs: 5,
        }
    }
}

/// Log aggregator — collects log entries and forwards them.
pub struct LogAggregator {
    config: AggregatorConfig,
    tx: mpsc::UnboundedSender<LogEntry>,
    buffer: Vec<LogEntry>,
}

impl LogAggregator {
    /// Create a new log aggregator.
    pub fn new(config: AggregatorConfig) -> Self {
        let (tx, _rx) = mpsc::unbounded_channel();
        Self {
            config,
            tx,
            buffer: Vec::new(),
        }
    }

    /// Submit a log entry.
    pub fn submit(&mut self, entry: LogEntry) {
        if entry.level >= self.config.min_level {
            self.buffer.push(entry);
            if self.buffer.len() >= self.config.buffer_size {
                self.flush();
            }
        }
    }

    /// Flush buffered entries.
    pub fn flush(&mut self) {
        // In production, this would forward to a log aggregation service
        // (e.g., Elasticsearch, Loki, CloudWatch).
        for entry in &self.buffer {
            let _ = self.tx.send(entry.clone());
        }
        self.buffer.clear();
    }

    /// Get buffered entries.
    pub fn buffered(&self) -> &[LogEntry] {
        &self.buffer
    }

    /// Get the configuration.
    pub fn config(&self) -> &AggregatorConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_and_buffer() {
        let mut agg = LogAggregator::new(AggregatorConfig {
            min_level: LogLevel::Info,
            buffer_size: 10,
            ..Default::default()
        });

        agg.submit(LogEntry::new(LogLevel::Info, "test"));
        assert_eq!(agg.buffered().len(), 1);
    }

    #[test]
    fn filter_by_level() {
        let mut agg = LogAggregator::new(AggregatorConfig {
            min_level: LogLevel::Warn,
            buffer_size: 10,
            ..Default::default()
        });

        agg.submit(LogEntry::new(LogLevel::Debug, "debug"));
        agg.submit(LogEntry::new(LogLevel::Info, "info"));
        agg.submit(LogEntry::new(LogLevel::Warn, "warn"));
        agg.submit(LogEntry::new(LogLevel::Error, "error"));

        assert_eq!(agg.buffered().len(), 2); // warn + error
    }

    #[test]
    fn flush_on_buffer_full() {
        let mut agg = LogAggregator::new(AggregatorConfig {
            min_level: LogLevel::Info,
            buffer_size: 3,
            ..Default::default()
        });

        agg.submit(LogEntry::new(LogLevel::Info, "a"));
        agg.submit(LogEntry::new(LogLevel::Info, "b"));
        assert_eq!(agg.buffered().len(), 2);

        agg.submit(LogEntry::new(LogLevel::Info, "c")); // triggers flush
        assert_eq!(agg.buffered().len(), 0);
    }
}
