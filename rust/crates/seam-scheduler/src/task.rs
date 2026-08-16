//! Scheduled task definition.

use serde::{Deserialize, Serialize};

/// Unique task identifier.
pub type TaskId = String;

/// Task priority.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    Critical = 0,
    High = 1,
    #[default]
    Normal = 2,
    Low = 3,
}

/// Task status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// A scheduled task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    /// Task ID.
    pub id: TaskId,

    /// Task name.
    pub name: String,

    /// Task type (e.g., "agent_turn", "tool_call", "workflow").
    pub task_type: String,

    /// Task payload.
    pub payload: serde_json::Value,

    /// Priority.
    pub priority: TaskPriority,

    /// Cron expression (for recurring tasks).
    pub cron: Option<String>,

    /// Delay in seconds (for one-shot delayed tasks).
    pub delay_secs: Option<u64>,

    /// Maximum retries on failure.
    pub max_retries: u32,

    /// Current status.
    pub status: TaskStatus,

    /// Number of retries attempted.
    pub retries: u32,
}

impl ScheduledTask {
    /// Create a new one-shot task.
    pub fn new(name: impl Into<String>, task_type: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            task_type: task_type.into(),
            payload: serde_json::json!({}),
            priority: TaskPriority::default(),
            cron: None,
            delay_secs: None,
            max_retries: 3,
            status: TaskStatus::Pending,
            retries: 0,
        }
    }

    /// Create a recurring task with a cron expression.
    pub fn recurring(name: impl Into<String>, cron: impl Into<String>) -> Self {
        Self {
            cron: Some(cron.into()),
            ..Self::new(name, "recurring")
        }
    }

    /// Create a delayed task.
    pub fn delayed(name: impl Into<String>, delay_secs: u64) -> Self {
        Self {
            delay_secs: Some(delay_secs),
            ..Self::new(name, "delayed")
        }
    }

    /// Set the payload.
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }

    /// Set the priority.
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set max retries.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_task() {
        let task = ScheduledTask::new("test", "agent_turn");
        assert_eq!(task.name, "test");
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.cron.is_none());
    }

    #[test]
    fn recurring_task() {
        let task = ScheduledTask::recurring("daily", "0 0 * * *");
        assert_eq!(task.cron, Some("0 0 * * *".into()));
    }

    #[test]
    fn delayed_task() {
        let task = ScheduledTask::delayed("later", 300);
        assert_eq!(task.delay_secs, Some(300));
    }

    #[test]
    fn task_builder() {
        let task = ScheduledTask::new("test", "agent")
            .with_payload(serde_json::json!({"prompt": "hello"}))
            .with_priority(TaskPriority::High)
            .with_max_retries(5);

        assert_eq!(task.priority, TaskPriority::High);
        assert_eq!(task.max_retries, 5);
    }
}
