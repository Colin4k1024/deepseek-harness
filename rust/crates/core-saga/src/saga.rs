//! Saga — execution engine with forward execution and rollback on failure.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::step::{SagaStep, StepResult};

/// Saga errors.
#[derive(Debug, Error)]
pub enum SagaError {
    #[error("step {step} failed: {reason}")]
    StepFailed { step: String, reason: String },
    #[error("compensation failed for step {step}: {reason}")]
    CompensationFailed { step: String, reason: String },
}

/// Saga execution status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SagaStatus {
    Pending,
    Running,
    Completed,
    Compensating,
    Compensated,
    Failed,
}

/// Result of a saga execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaResult {
    /// Final status.
    pub status: SagaStatus,

    /// Step results (forward execution).
    pub step_results: Vec<(String, StepResult)>,

    /// Compensation results (if rolled back).
    pub compensation_results: Vec<(String, StepResult)>,
}

/// Saga — orchestrates multi-step distributed transactions.
pub struct Saga {
    name: String,
    steps: Vec<Box<dyn SagaStep>>,
}

impl Saga {
    /// Create a new saga.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            steps: Vec::new(),
        }
    }

    /// Add a step to the saga.
    pub fn add_step(&mut self, step: impl SagaStep + 'static) {
        self.steps.push(Box::new(step));
    }

    /// Execute the saga.
    ///
    /// Runs each step in order. If any step fails, compensations
    /// are executed in reverse order.
    pub async fn execute(&self, initial_input: serde_json::Value) -> SagaResult {
        let mut step_results = Vec::new();
        let mut completed_steps = Vec::new();
        let mut current_input = initial_input;

        // Forward execution
        for step in &self.steps {
            tracing::info!(saga = %self.name, step = step.name(), "executing saga step");

            let result = step.execute(&current_input).await;

            if result.success {
                completed_steps.push(step.name().to_string());
                step_results.push((step.name().to_string(), result.clone()));
                current_input = result.output;
            } else {
                // Step failed — compensate
                tracing::warn!(
                    saga = %self.name,
                    step = step.name(),
                    error = ?result.error,
                    "saga step failed, starting compensation"
                );

                step_results.push((step.name().to_string(), result));

                let compensation_results = self.compensate(&completed_steps, &current_input).await;

                return SagaResult {
                    status: SagaStatus::Compensated,
                    step_results,
                    compensation_results,
                };
            }
        }

        tracing::info!(saga = %self.name, "saga completed successfully");
        SagaResult {
            status: SagaStatus::Completed,
            step_results,
            compensation_results: Vec::new(),
        }
    }

    /// Execute compensations in reverse order.
    async fn compensate(
        &self,
        completed_steps: &[String],
        _input: &serde_json::Value,
    ) -> Vec<(String, StepResult)> {
        let mut results = Vec::new();

        for step_name in completed_steps.iter().rev() {
            if let Some(step) = self.steps.iter().find(|s| s.name() == *step_name) {
                tracing::info!(saga = %self.name, step = step_name, "compensating");
                let result = step.compensate(&serde_json::json!({})).await;
                results.push((step_name.clone(), result));
            }
        }

        results
    }

    /// Get the saga name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the number of steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::super::step::ClosureStep;
    use super::*;

    #[tokio::test]
    async fn successful_saga() {
        let mut saga = Saga::new("test");
        saga.add_step(ClosureStep::new(
            "step1",
            |_| StepResult {
                success: true,
                output: serde_json::json!({"step1": true}),
                error: None,
            },
            |_| StepResult {
                success: true,
                output: serde_json::json!({"undone": 1}),
                error: None,
            },
        ));
        saga.add_step(ClosureStep::new(
            "step2",
            |_| StepResult {
                success: true,
                output: serde_json::json!({"step2": true}),
                error: None,
            },
            |_| StepResult {
                success: true,
                output: serde_json::json!({"undone": 2}),
                error: None,
            },
        ));

        let result = saga.execute(serde_json::json!({})).await;
        assert_eq!(result.status, SagaStatus::Completed);
        assert_eq!(result.step_results.len(), 2);
        assert!(result.compensation_results.is_empty());
    }

    #[tokio::test]
    async fn failed_saga_triggers_compensation() {
        let mut saga = Saga::new("test");
        saga.add_step(ClosureStep::new(
            "step1",
            |_| StepResult {
                success: true,
                output: serde_json::json!({"step1": true}),
                error: None,
            },
            |_| StepResult {
                success: true,
                output: serde_json::json!({"compensated": "step1"}),
                error: None,
            },
        ));
        saga.add_step(ClosureStep::new(
            "step2_fail",
            |_| StepResult {
                success: false,
                output: serde_json::json!(null),
                error: Some("intentional failure".into()),
            },
            |_| StepResult {
                success: true,
                output: serde_json::json!({"compensated": "step2"}),
                error: None,
            },
        ));

        let result = saga.execute(serde_json::json!({})).await;
        assert_eq!(result.status, SagaStatus::Compensated);
        assert_eq!(result.step_results.len(), 2); // step1 success + step2 failure
        assert_eq!(result.compensation_results.len(), 1); // step1 compensated
    }
}
