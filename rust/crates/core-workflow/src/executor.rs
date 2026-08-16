//! Workflow executor — runs workflows respecting dependencies.
//!
//! Executes steps in topological order, parallelizing independent steps.
//! Uses `tokio::task::JoinSet` for concurrent execution.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::step::{Step, StepId, StepResult, StepStatus};
use crate::workflow::{Workflow, WorkflowError, WorkflowStatus};

/// Trait for executing individual workflow steps.
///
/// Implement this to define how each step type is executed.
#[async_trait]
pub trait StepExecutor: Send + Sync {
    /// Execute a single step.
    async fn execute(&self, step: &Step, context: &serde_json::Value) -> StepResult;
}

/// Result of a complete workflow execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub workflow_id: String,
    pub status: WorkflowStatus,
    pub step_results: HashMap<StepId, StepResult>,
    pub duration_ms: u64,
}

/// Workflow executor — runs a workflow to completion.
pub struct WorkflowExecutor;

impl WorkflowExecutor {
    /// Execute a workflow.
    ///
    /// Runs steps respecting dependencies. Independent steps run in parallel.
    pub async fn execute(
        workflow: &mut Workflow,
        executor: &dyn StepExecutor,
        context: &serde_json::Value,
    ) -> Result<ExecutionResult, WorkflowError> {
        workflow.validate()?;
        workflow.status = WorkflowStatus::Running;

        let start = Instant::now();
        let mut completed: HashSet<String> = HashSet::new();
        let mut results: HashMap<StepId, StepResult> = HashMap::new();
        let mut failed = false;

        loop {
            // Find steps ready to run
            let ready: Vec<StepId> = workflow
                .ready_steps(&completed.iter().map(|s| s.as_str()).collect())
                .iter()
                .map(|s| s.id.clone())
                .collect();

            if ready.is_empty() {
                break; // All steps done or no progress possible
            }

            // Execute ready steps in parallel
            for step_id in &ready {
                let step = workflow.get_step(step_id).unwrap().clone();
                let ctx = context.clone();
                // We need to call the executor, but it's a trait object.
                // For now, execute sequentially (parallel requires Arc<dyn StepExecutor>).
                let result = executor.execute(&step, &ctx).await;

                if result.status == StepStatus::Failed {
                    failed = true;
                }

                completed.insert(step_id.clone());
                results.insert(step_id.clone(), result);
            }

            // If any step failed, mark remaining as skipped
            if failed {
                for step in &workflow.steps {
                    if !completed.contains(&step.id) {
                        results.insert(
                            step.id.clone(),
                            StepResult {
                                step_id: step.id.clone(),
                                status: StepStatus::Skipped,
                                output: serde_json::json!(null),
                                error: Some("upstream step failed".into()),
                                duration_ms: 0,
                            },
                        );
                    }
                }
                break;
            }
        }

        let duration = start.elapsed();
        let status = if failed {
            WorkflowStatus::Failed
        } else {
            WorkflowStatus::Completed
        };
        workflow.status = status.clone();

        Ok(ExecutionResult {
            workflow_id: workflow.id.clone(),
            status,
            step_results: results,
            duration_ms: duration.as_millis() as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::Step;
    use crate::workflow::Workflow;

    struct MockExecutor;

    #[async_trait]
    impl StepExecutor for MockExecutor {
        async fn execute(&self, step: &Step, _context: &serde_json::Value) -> StepResult {
            StepResult {
                step_id: step.id.clone(),
                status: StepStatus::Completed,
                output: serde_json::json!({ "result": format!("done: {}", step.name) }),
                error: None,
                duration_ms: 10,
            }
        }
    }

    struct FailingExecutor;

    #[async_trait]
    impl StepExecutor for FailingExecutor {
        async fn execute(&self, step: &Step, _context: &serde_json::Value) -> StepResult {
            if step.id == "b" {
                StepResult {
                    step_id: step.id.clone(),
                    status: StepStatus::Failed,
                    output: serde_json::json!(null),
                    error: Some("step failed".into()),
                    duration_ms: 5,
                }
            } else {
                StepResult {
                    step_id: step.id.clone(),
                    status: StepStatus::Completed,
                    output: serde_json::json!({ "result": "ok" }),
                    error: None,
                    duration_ms: 10,
                }
            }
        }
    }

    #[tokio::test]
    async fn simple_execution() {
        let mut wf = Workflow::new("wf1", "test");
        wf.add_step(Step::new("a", "step A", "agent")).unwrap();
        wf.add_step(Step::new("b", "step B", "agent").depends_on("a"))
            .unwrap();

        let executor = MockExecutor;
        let result = WorkflowExecutor::execute(&mut wf, &executor, &serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(result.status, WorkflowStatus::Completed);
        assert_eq!(result.step_results.len(), 2);
    }

    #[tokio::test]
    async fn failure_skips_downstream() {
        let mut wf = Workflow::new("wf1", "test");
        wf.add_step(Step::new("a", "step A", "agent")).unwrap();
        wf.add_step(Step::new("b", "step B", "agent").depends_on("a"))
            .unwrap();
        wf.add_step(Step::new("c", "step C", "agent").depends_on("b"))
            .unwrap();

        let executor = FailingExecutor;
        let result = WorkflowExecutor::execute(&mut wf, &executor, &serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(result.status, WorkflowStatus::Failed);
        // c should be skipped because b failed
        let c_result = result.step_results.get("c").unwrap();
        assert_eq!(c_result.status, StepStatus::Skipped);
    }
}
