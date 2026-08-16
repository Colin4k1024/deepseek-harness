//! Step — a single unit of work in a workflow.

use serde::{Deserialize, Serialize};

/// Unique step identifier.
pub type StepId = String;

/// A single step in a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Unique step ID.
    pub id: StepId,

    /// Human-readable name.
    pub name: String,

    /// Step type (e.g., "agent", "tool", "transform").
    pub step_type: String,

    /// Input configuration (arbitrary JSON).
    pub input: serde_json::Value,

    /// IDs of steps that must complete before this one runs.
    pub depends_on: Vec<StepId>,

    /// Timeout in seconds (0 = no timeout).
    pub timeout_secs: u64,
}

impl Step {
    /// Create a new step.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        step_type: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            step_type: step_type.into(),
            input: serde_json::json!({}),
            depends_on: Vec::new(),
            timeout_secs: 0,
        }
    }

    /// Set input configuration.
    pub fn with_input(mut self, input: serde_json::Value) -> Self {
        self.input = input;
        self
    }

    /// Add a dependency.
    pub fn depends_on(mut self, step_id: impl Into<String>) -> Self {
        self.depends_on.push(step_id.into());
        self
    }

    /// Set timeout.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

/// Status of a step execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

/// Result of a step execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: StepId,
    pub status: StepStatus,
    pub output: serde_json::Value,
    pub error: Option<String>,
    pub duration_ms: u64,
}
