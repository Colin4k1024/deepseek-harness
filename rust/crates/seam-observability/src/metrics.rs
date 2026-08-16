//! Metrics collection — counters, histograms, and gauges for agent operations.
//!
//! Tracks key metrics:
//! - Request counts (by type, status)
//! - Latency histograms (LLM calls, tool execution, end-to-end)
//! - Active sessions gauge
//! - Token usage counters

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Metrics collector for agent operations.
///
/// Thread-safe, lock-free counters for high-throughput metrics.
/// Histograms use a simple bucket-based approach.
#[derive(Clone)]
pub struct Metrics {
    inner: Arc<MetricsInner>,
}

struct MetricsInner {
    counters: RwLock<HashMap<String, AtomicU64>>,
    histograms: RwLock<HashMap<String, Histogram>>,
    gauges: RwLock<HashMap<String, AtomicU64>>,
}

struct Histogram {
    buckets: Vec<f64>,
    counts: Vec<AtomicU64>,
    sum: AtomicU64,
    count: AtomicU64,
}

impl Histogram {
    fn new(buckets: Vec<f64>) -> Self {
        let counts = buckets.iter().map(|_| AtomicU64::new(0)).collect();
        Self {
            buckets,
            counts,
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    fn record(&self, value: f64) {
        self.sum.fetch_add(value as u64, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
        for (i, &bucket) in self.buckets.iter().enumerate() {
            if value <= bucket {
                self.counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

impl Metrics {
    /// Create a new metrics collector.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MetricsInner {
                counters: RwLock::new(HashMap::new()),
                histograms: RwLock::new(HashMap::new()),
                gauges: RwLock::new(HashMap::new()),
            }),
        }
    }

    // --- Counters ---

    /// Increment a counter by 1.
    pub fn inc(&self, name: &str) {
        self.inc_by(name, 1);
    }

    /// Increment a counter by a value.
    pub fn inc_by(&self, name: &str, value: u64) {
        let counters = self.inner.counters.read().unwrap();
        if let Some(counter) = counters.get(name) {
            counter.fetch_add(value, Ordering::Relaxed);
        } else {
            drop(counters);
            let mut counters = self.inner.counters.write().unwrap();
            counters
                .entry(name.to_string())
                .or_insert_with(|| AtomicU64::new(0))
                .fetch_add(value, Ordering::Relaxed);
        }
    }

    /// Get counter value.
    pub fn counter(&self, name: &str) -> u64 {
        let counters = self.inner.counters.read().unwrap();
        counters
            .get(name)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    // --- Histograms ---

    /// Record a duration in a histogram.
    pub fn record_duration(&self, name: &str, duration: Duration) {
        self.record_value(name, duration.as_millis() as f64);
    }

    /// Record a value in a histogram.
    pub fn record_value(&self, name: &str, value: f64) {
        let histograms = self.inner.histograms.read().unwrap();
        if let Some(hist) = histograms.get(name) {
            hist.record(value);
        } else {
            drop(histograms);
            // Default buckets: 1ms, 5ms, 10ms, 50ms, 100ms, 500ms, 1s, 5s, 10s, 30s
            let default_buckets = vec![
                1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0, 10000.0, 30000.0,
            ];
            let mut histograms = self.inner.histograms.write().unwrap();
            histograms
                .entry(name.to_string())
                .or_insert_with(|| Histogram::new(default_buckets))
                .record(value);
        }
    }

    // --- Gauges ---

    /// Set a gauge value.
    pub fn set_gauge(&self, name: &str, value: u64) {
        let gauges = self.inner.gauges.read().unwrap();
        if let Some(gauge) = gauges.get(name) {
            gauge.store(value, Ordering::Relaxed);
        } else {
            drop(gauges);
            let mut gauges = self.inner.gauges.write().unwrap();
            gauges
                .entry(name.to_string())
                .or_insert_with(|| AtomicU64::new(0))
                .store(value, Ordering::Relaxed);
        }
    }

    /// Increment a gauge.
    pub fn inc_gauge(&self, name: &str) {
        let gauges = self.inner.gauges.read().unwrap();
        if let Some(gauge) = gauges.get(name) {
            gauge.fetch_add(1, Ordering::Relaxed);
        } else {
            drop(gauges);
            let mut gauges = self.inner.gauges.write().unwrap();
            gauges
                .entry(name.to_string())
                .or_insert_with(|| AtomicU64::new(0))
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Decrement a gauge.
    pub fn dec_gauge(&self, name: &str) {
        let gauges = self.inner.gauges.read().unwrap();
        if let Some(gauge) = gauges.get(name) {
            gauge.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// Get gauge value.
    pub fn gauge(&self, name: &str) -> u64 {
        let gauges = self.inner.gauges.read().unwrap();
        gauges
            .get(name)
            .map(|g| g.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    // --- Convenience methods for common agent metrics ---

    /// Record an LLM request.
    pub fn record_llm_request(&self, model: &str, duration: Duration, tokens: u64) {
        self.inc("llm.requests.total");
        self.inc(&format!("llm.requests.model.{model}"));
        self.record_duration("llm.latency_ms", duration);
        self.inc_by("llm.tokens.total", tokens);
    }

    /// Record a tool call.
    pub fn record_tool_call(&self, tool_name: &str, duration: Duration, success: bool) {
        self.inc("tool.calls.total");
        self.inc(&format!("tool.calls.{tool_name}"));
        self.record_duration("tool.latency_ms", duration);
        if success {
            self.inc("tool.calls.success");
        } else {
            self.inc("tool.calls.error");
        }
    }

    /// Record a session event.
    pub fn record_session_event(&self, event_type: &str) {
        self.inc("session.events.total");
        self.inc(&format!("session.events.{event_type}"));
    }

    /// Record an agent turn.
    pub fn record_agent_turn(&self, duration: Duration, tool_calls: u64) {
        self.inc("agent.turns.total");
        self.record_duration("agent.turn_duration_ms", duration);
        self.inc_by("agent.tool_calls_per_turn", tool_calls);
    }

    /// Export all metrics as a JSON snapshot.
    pub fn snapshot(&self) -> serde_json::Value {
        let counters = self.inner.counters.read().unwrap();
        let gauges = self.inner.gauges.read().unwrap();

        let counter_map: HashMap<String, u64> = counters
            .iter()
            .map(|(k, v)| (k.clone(), v.load(Ordering::Relaxed)))
            .collect();

        let gauge_map: HashMap<String, u64> = gauges
            .iter()
            .map(|(k, v)| (k.clone(), v.load(Ordering::Relaxed)))
            .collect();

        serde_json::json!({
            "counters": counter_map,
            "gauges": gauge_map,
        })
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Timer guard — records duration on drop.
pub struct TimerGuard {
    metrics: Metrics,
    name: String,
    start: Instant,
}

impl TimerGuard {
    pub fn new(metrics: Metrics, name: impl Into<String>) -> Self {
        Self {
            metrics,
            name: name.into(),
            start: Instant::now(),
        }
    }
}

impl Drop for TimerGuard {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        self.metrics.record_duration(&self.name, duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_inc() {
        let m = Metrics::new();
        assert_eq!(m.counter("test"), 0);
        m.inc("test");
        assert_eq!(m.counter("test"), 1);
        m.inc_by("test", 5);
        assert_eq!(m.counter("test"), 6);
    }

    #[test]
    fn gauge_ops() {
        let m = Metrics::new();
        m.set_gauge("active", 10);
        assert_eq!(m.gauge("active"), 10);
        m.inc_gauge("active");
        assert_eq!(m.gauge("active"), 11);
        m.dec_gauge("active");
        assert_eq!(m.gauge("active"), 10);
    }

    #[test]
    fn llm_metrics() {
        let m = Metrics::new();
        m.record_llm_request("deepseek-chat", Duration::from_millis(500), 100);
        assert_eq!(m.counter("llm.requests.total"), 1);
        assert_eq!(m.counter("llm.tokens.total"), 100);
    }

    #[test]
    fn tool_metrics() {
        let m = Metrics::new();
        m.record_tool_call("echo", Duration::from_millis(10), true);
        m.record_tool_call("echo", Duration::from_millis(5), false);
        assert_eq!(m.counter("tool.calls.total"), 2);
        assert_eq!(m.counter("tool.calls.success"), 1);
        assert_eq!(m.counter("tool.calls.error"), 1);
    }

    #[test]
    fn timer_guard() {
        let m = Metrics::new();
        {
            let _timer = TimerGuard::new(m.clone(), "test_timer");
            std::thread::sleep(Duration::from_millis(10));
        }
        // Timer should have recorded the duration
        // (we can't check exact value, but the histogram should exist)
    }

    #[test]
    fn snapshot_json() {
        let m = Metrics::new();
        m.inc("requests");
        m.set_gauge("active_sessions", 5);

        let snap = m.snapshot();
        assert_eq!(snap["counters"]["requests"], 1);
        assert_eq!(snap["gauges"]["active_sessions"], 5);
    }
}
