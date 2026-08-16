//! Resource monitoring — tracks current resource usage.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Current resource usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Current memory usage in bytes.
    pub memory_bytes: u64,

    /// Current CPU usage percentage.
    pub cpu_percent: u32,

    /// Current file descriptor count.
    pub file_descriptors: u32,

    /// Current concurrent task count.
    pub concurrent_tasks: u32,

    /// Current connection count.
    pub connections: u32,
}

/// Resource monitor — tracks current resource usage.
pub struct ResourceMonitor {
    memory_bytes: AtomicU64,
    cpu_percent: AtomicU32,
    file_descriptors: AtomicU32,
    concurrent_tasks: AtomicU32,
    connections: AtomicU32,
}

impl ResourceMonitor {
    /// Create a new resource monitor.
    pub fn new() -> Self {
        Self {
            memory_bytes: AtomicU64::new(0),
            cpu_percent: AtomicU32::new(0),
            file_descriptors: AtomicU32::new(0),
            concurrent_tasks: AtomicU32::new(0),
            connections: AtomicU32::new(0),
        }
    }

    /// Update memory usage.
    pub fn set_memory(&self, bytes: u64) {
        self.memory_bytes.store(bytes, Ordering::Relaxed);
    }

    /// Update CPU usage.
    pub fn set_cpu(&self, percent: u32) {
        self.cpu_percent.store(percent, Ordering::Relaxed);
    }

    /// Increment concurrent tasks.
    pub fn task_start(&self) {
        self.concurrent_tasks.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement concurrent tasks.
    pub fn task_end(&self) {
        self.concurrent_tasks.fetch_sub(1, Ordering::Relaxed);
    }

    /// Increment connections.
    pub fn connection_open(&self) {
        self.connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement connections.
    pub fn connection_close(&self) {
        self.connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get current usage.
    pub fn usage(&self) -> ResourceUsage {
        ResourceUsage {
            memory_bytes: self.memory_bytes.load(Ordering::Relaxed),
            cpu_percent: self.cpu_percent.load(Ordering::Relaxed),
            file_descriptors: self.file_descriptors.load(Ordering::Relaxed),
            concurrent_tasks: self.concurrent_tasks.load(Ordering::Relaxed),
            connections: self.connections.load(Ordering::Relaxed),
        }
    }
}

impl Default for ResourceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_usage() {
        let monitor = ResourceMonitor::new();
        monitor.set_memory(1024);
        monitor.set_cpu(50);
        monitor.task_start();
        monitor.connection_open();

        let usage = monitor.usage();
        assert_eq!(usage.memory_bytes, 1024);
        assert_eq!(usage.cpu_percent, 50);
        assert_eq!(usage.concurrent_tasks, 1);
        assert_eq!(usage.connections, 1);
    }

    #[test]
    fn task_lifecycle() {
        let monitor = ResourceMonitor::new();
        monitor.task_start();
        monitor.task_start();
        assert_eq!(monitor.usage().concurrent_tasks, 2);
        monitor.task_end();
        assert_eq!(monitor.usage().concurrent_tasks, 1);
    }
}
