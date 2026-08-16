//! Throughput tracking — requests per second, tokens per second.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Throughput statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputStats {
    /// Total requests.
    pub total_requests: u64,

    /// Requests per second (over last window).
    pub rps: f64,

    /// Total tokens processed.
    pub total_tokens: u64,

    /// Tokens per second (over last window).
    pub tps: f64,

    /// Active connections.
    pub active_connections: u64,
}

/// Throughput tracker — tracks request and token throughput.
pub struct ThroughputTracker {
    total_requests: AtomicU64,
    total_tokens: AtomicU64,
    active_connections: AtomicU64,
    window_start: Mutex<Instant>,
    window_requests: AtomicU64,
    window_tokens: AtomicU64,
    window_secs: u64,
}

impl ThroughputTracker {
    /// Create a new throughput tracker.
    pub fn new(window_secs: u64) -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            total_tokens: AtomicU64::new(0),
            active_connections: AtomicU64::new(0),
            window_start: Mutex::new(Instant::now()),
            window_requests: AtomicU64::new(0),
            window_tokens: AtomicU64::new(0),
            window_secs,
        }
    }

    /// Record a request.
    pub fn record_request(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.window_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Record tokens processed.
    pub fn record_tokens(&self, count: u64) {
        self.total_tokens.fetch_add(count, Ordering::Relaxed);
        self.window_tokens.fetch_add(count, Ordering::Relaxed);
    }

    /// Increment active connections.
    pub fn connect(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active connections.
    pub fn disconnect(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get current statistics.
    pub fn stats(&self) -> ThroughputStats {
        let mut start = self.window_start.lock().unwrap();
        let elapsed = start.elapsed().as_secs_f64();

        let rps = if elapsed > 0.0 {
            self.window_requests.load(Ordering::Relaxed) as f64 / elapsed
        } else {
            0.0
        };

        let tps = if elapsed > 0.0 {
            self.window_tokens.load(Ordering::Relaxed) as f64 / elapsed
        } else {
            0.0
        };

        // Reset window if elapsed
        if elapsed >= self.window_secs as f64 {
            *start = Instant::now();
            self.window_requests.store(0, Ordering::Relaxed);
            self.window_tokens.store(0, Ordering::Relaxed);
        }

        ThroughputStats {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            rps,
            total_tokens: self.total_tokens.load(Ordering::Relaxed),
            tps,
            active_connections: self.active_connections.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_requests() {
        let tracker = ThroughputTracker::new(60);
        for _ in 0..10 {
            tracker.record_request();
        }
        let stats = tracker.stats();
        assert_eq!(stats.total_requests, 10);
        assert!(stats.rps > 0.0);
    }

    #[test]
    fn record_tokens() {
        let tracker = ThroughputTracker::new(60);
        tracker.record_tokens(100);
        tracker.record_tokens(200);
        let stats = tracker.stats();
        assert_eq!(stats.total_tokens, 300);
    }

    #[test]
    fn active_connections() {
        let tracker = ThroughputTracker::new(60);
        tracker.connect();
        tracker.connect();
        assert_eq!(tracker.stats().active_connections, 2);
        tracker.disconnect();
        assert_eq!(tracker.stats().active_connections, 1);
    }
}
