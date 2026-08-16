//! Latency tracking with percentile calculation.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Duration;

/// Latency statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStats {
    /// Total number of samples.
    pub count: u64,

    /// Minimum latency in microseconds.
    pub min_us: u64,

    /// Maximum latency in microseconds.
    pub max_us: u64,

    /// Mean latency in microseconds.
    pub mean_us: u64,

    /// P50 latency in microseconds.
    pub p50_us: u64,

    /// P90 latency in microseconds.
    pub p90_us: u64,

    /// P95 latency in microseconds.
    pub p95_us: u64,

    /// P99 latency in microseconds.
    pub p99_us: u64,
}

/// Latency tracker — records latency samples and computes percentiles.
pub struct LatencyTracker {
    name: String,
    samples: Mutex<Vec<u64>>,
    max_samples: usize,
}

impl LatencyTracker {
    /// Create a new latency tracker.
    pub fn new(name: impl Into<String>, max_samples: usize) -> Self {
        Self {
            name: name.into(),
            samples: Mutex::new(Vec::new()),
            max_samples,
        }
    }

    /// Record a latency sample.
    pub fn record(&self, duration: Duration) {
        let mut samples = self.samples.lock().unwrap();
        if samples.len() >= self.max_samples {
            samples.remove(0); // FIFO eviction
        }
        samples.push(duration.as_micros() as u64);
    }

    /// Get current statistics.
    pub fn stats(&self) -> LatencyStats {
        let samples = self.samples.lock().unwrap();
        if samples.is_empty() {
            return LatencyStats {
                count: 0,
                min_us: 0,
                max_us: 0,
                mean_us: 0,
                p50_us: 0,
                p90_us: 0,
                p95_us: 0,
                p99_us: 0,
            };
        }

        let mut sorted = samples.clone();
        sorted.sort_unstable();

        let count = sorted.len() as u64;
        let min_us = sorted[0];
        let max_us = sorted[sorted.len() - 1];
        let mean_us = sorted.iter().sum::<u64>() / count;
        let p50_us = percentile(&sorted, 50);
        let p90_us = percentile(&sorted, 90);
        let p95_us = percentile(&sorted, 95);
        let p99_us = percentile(&sorted, 99);

        LatencyStats {
            count,
            min_us,
            max_us,
            mean_us,
            p50_us,
            p90_us,
            p95_us,
            p99_us,
        }
    }

    /// Get the tracker name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Clear all samples.
    pub fn clear(&self) {
        self.samples.lock().unwrap().clear();
    }
}

fn percentile(sorted: &[u64], p: u64) -> u64 {
    let idx = (p as usize * sorted.len()) / 100;
    sorted[idx.min(sorted.len() - 1)]
}

/// Timer guard — records latency on drop.
pub struct LatencyGuard<'a> {
    tracker: &'a LatencyTracker,
    start: std::time::Instant,
}

impl<'a> LatencyGuard<'a> {
    pub fn new(tracker: &'a LatencyTracker) -> Self {
        Self {
            tracker,
            start: std::time::Instant::now(),
        }
    }
}

impl Drop for LatencyGuard<'_> {
    fn drop(&mut self) {
        self.tracker.record(self.start.elapsed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_stats() {
        let tracker = LatencyTracker::new("test", 1000);
        for i in 1..=100 {
            tracker.record(Duration::from_micros(i));
        }

        let stats = tracker.stats();
        assert_eq!(stats.count, 100);
        assert_eq!(stats.min_us, 1);
        assert_eq!(stats.max_us, 100);
        assert!(stats.mean_us > 0);
        assert!(stats.p50_us > 0);
        assert!(stats.p90_us > 0);
    }

    #[test]
    fn fifo_eviction() {
        let tracker = LatencyTracker::new("test", 10);
        for i in 1..=20 {
            tracker.record(Duration::from_micros(i));
        }

        let stats = tracker.stats();
        assert_eq!(stats.count, 10);
        assert_eq!(stats.min_us, 11); // first 10 evicted
    }

    #[test]
    fn empty_stats() {
        let tracker = LatencyTracker::new("test", 100);
        let stats = tracker.stats();
        assert_eq!(stats.count, 0);
    }

    #[test]
    fn latency_guard() {
        let tracker = LatencyTracker::new("test", 100);
        {
            let _guard = LatencyGuard::new(&tracker);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(tracker.stats().count, 1);
    }
}
