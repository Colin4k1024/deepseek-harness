//! Scheduler — manages task execution with priority queue.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Mutex;

use super::task::{ScheduledTask, TaskStatus};

/// Scheduler configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    /// Maximum concurrent tasks.
    pub max_concurrent: u32,

    /// Poll interval in milliseconds.
    pub poll_interval_ms: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 10,
            poll_interval_ms: 100,
        }
    }
}

/// Wrapper for priority queue ordering.
struct TaskEntry {
    task: ScheduledTask,
    scheduled_at: u64,
}

impl PartialEq for TaskEntry {
    fn eq(&self, other: &Self) -> bool {
        self.task.priority == other.task.priority && self.scheduled_at == other.scheduled_at
    }
}

impl Eq for TaskEntry {}

impl PartialOrd for TaskEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TaskEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap, so we reverse the priority comparison
        // Lower priority number = higher priority (Critical=0 is highest)
        // For equal priority, earlier scheduled_at wins
        other
            .task
            .priority
            .cmp(&self.task.priority)
            .then_with(|| self.scheduled_at.cmp(&other.scheduled_at))
    }
}

/// Scheduler — manages and executes scheduled tasks.
pub struct Scheduler {
    #[allow(dead_code)]
    config: SchedulerConfig,
    queue: Mutex<BinaryHeap<TaskEntry>>,
    completed: Mutex<Vec<ScheduledTask>>,
}

impl Scheduler {
    /// Create a new scheduler.
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            config,
            queue: Mutex::new(BinaryHeap::new()),
            completed: Mutex::new(Vec::new()),
        }
    }

    /// Submit a task for scheduling.
    pub fn submit(&self, task: ScheduledTask) {
        let entry = TaskEntry {
            task,
            scheduled_at: now(),
        };
        self.queue.lock().unwrap().push(entry);
    }

    /// Get the next task to execute.
    pub fn next(&self) -> Option<ScheduledTask> {
        self.queue.lock().unwrap().pop().map(|e| e.task)
    }

    /// Mark a task as completed.
    pub fn complete(&self, task: ScheduledTask) {
        let mut completed = self.completed.lock().unwrap();
        let mut t = task;
        t.status = TaskStatus::Completed;
        completed.push(t);
    }

    /// Mark a task as failed. Re-queues if retries remain.
    pub fn fail(&self, task: ScheduledTask) {
        let mut t = task;
        t.retries += 1;
        if t.retries < t.max_retries {
            t.status = TaskStatus::Pending;
            self.submit(t);
        } else {
            t.status = TaskStatus::Failed;
            self.completed.lock().unwrap().push(t);
        }
    }

    /// Get queue length.
    pub fn pending_count(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    /// Get completed task count.
    pub fn completed_count(&self) -> usize {
        self.completed.lock().unwrap().len()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new(SchedulerConfig::default())
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
    use super::super::task::TaskPriority;
    use super::*;

    #[test]
    fn submit_and_next() {
        let scheduler = Scheduler::default();
        scheduler.submit(ScheduledTask::new("t1", "test"));
        scheduler.submit(ScheduledTask::new("t2", "test"));

        assert_eq!(scheduler.pending_count(), 2);
        let task = scheduler.next().unwrap();
        assert_eq!(task.name, "t1");
        assert_eq!(scheduler.pending_count(), 1);
    }

    #[test]
    fn priority_ordering() {
        let scheduler = Scheduler::default();
        scheduler.submit(ScheduledTask::new("low", "test").with_priority(TaskPriority::Low));
        scheduler.submit(ScheduledTask::new("high", "test").with_priority(TaskPriority::High));
        scheduler.submit(ScheduledTask::new("normal", "test"));

        assert_eq!(scheduler.next().unwrap().name, "high");
        assert_eq!(scheduler.next().unwrap().name, "normal");
        assert_eq!(scheduler.next().unwrap().name, "low");
    }

    #[test]
    fn complete_task() {
        let scheduler = Scheduler::default();
        scheduler.submit(ScheduledTask::new("t1", "test"));

        let task = scheduler.next().unwrap();
        scheduler.complete(task);

        assert_eq!(scheduler.completed_count(), 1);
        assert_eq!(scheduler.pending_count(), 0);
    }

    #[test]
    fn fail_and_retry() {
        let scheduler = Scheduler::default();
        let task = ScheduledTask::new("t1", "test").with_max_retries(3);

        scheduler.submit(task);
        let task = scheduler.next().unwrap();
        scheduler.fail(task); // retry 1

        assert_eq!(scheduler.pending_count(), 1); // re-queued
        assert_eq!(scheduler.completed_count(), 0);

        let task = scheduler.next().unwrap();
        scheduler.fail(task); // retry 2

        let task = scheduler.next().unwrap();
        scheduler.fail(task); // retry 3 — max reached

        assert_eq!(scheduler.pending_count(), 0);
        assert_eq!(scheduler.completed_count(), 1); // failed
    }
}
