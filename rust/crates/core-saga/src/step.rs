//! Saga step — a single step with forward action and compensation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Result of a saga step execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Whether the step succeeded.
    pub success: bool,

    /// Output data from the step.
    pub output: serde_json::Value,

    /// Error message if failed.
    pub error: Option<String>,
}

/// Trait for saga steps.
///
/// Each step has a forward action and a compensating action.
/// The compensation undoes the forward action if a later step fails.
#[async_trait]
pub trait SagaStep: Send + Sync {
    /// Step name for logging.
    fn name(&self) -> &str;

    /// Execute the forward action.
    async fn execute(&self, input: &serde_json::Value) -> StepResult;

    /// Execute the compensating action (undo).
    async fn compensate(&self, input: &serde_json::Value) -> StepResult;
}

/// A closure-based saga step.
pub struct ClosureStep {
    step_name: String,
    forward: Box<dyn Fn(serde_json::Value) -> StepResult + Send + Sync>,
    backward: Box<dyn Fn(serde_json::Value) -> StepResult + Send + Sync>,
}

impl ClosureStep {
    pub fn new(
        name: impl Into<String>,
        forward: impl Fn(serde_json::Value) -> StepResult + Send + Sync + 'static,
        backward: impl Fn(serde_json::Value) -> StepResult + Send + Sync + 'static,
    ) -> Self {
        Self {
            step_name: name.into(),
            forward: Box::new(forward),
            backward: Box::new(backward),
        }
    }
}

#[async_trait]
impl SagaStep for ClosureStep {
    fn name(&self) -> &str {
        &self.step_name
    }

    async fn execute(&self, input: &serde_json::Value) -> StepResult {
        (self.forward)(input.clone())
    }

    async fn compensate(&self, input: &serde_json::Value) -> StepResult {
        (self.backward)(input.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn closure_step() {
        let step = ClosureStep::new(
            "test",
            |input| StepResult {
                success: true,
                output: serde_json::json!({"echo": input}),
                error: None,
            },
            |_input| StepResult {
                success: true,
                output: serde_json::json!({"undone": true}),
                error: None,
            },
        );

        assert_eq!(step.name(), "test");

        let result = step.execute(&serde_json::json!("hello")).await;
        assert!(result.success);

        let result = step.compensate(&serde_json::json!({})).await;
        assert!(result.success);
        assert_eq!(result.output["undone"], true);
    }
}
