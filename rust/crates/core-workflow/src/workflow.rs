//! Workflow — a DAG of steps with dependencies.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::step::Step;

/// Errors from workflow operations.
#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("duplicate step ID: {0}")]
    DuplicateStep(String),
    #[error("step not found: {0}")]
    StepNotFound(String),
    #[error("dependency not found: {0} (referenced by step {1})")]
    DependencyNotFound(String, String),
    #[error("cycle detected in workflow DAG")]
    CycleDetected,
}

/// Status of a workflow execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowStatus {
    /// Workflow has not started.
    Pending,
    /// Workflow is running.
    Running,
    /// All steps completed successfully.
    Completed,
    /// One or more steps failed.
    Failed,
    /// Workflow was cancelled.
    Cancelled,
}

/// A workflow — a directed acyclic graph of steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// Workflow ID.
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Steps in the workflow.
    pub steps: Vec<Step>,

    /// Current status.
    pub status: WorkflowStatus,
}

impl Workflow {
    /// Create a new workflow.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            steps: Vec::new(),
            status: WorkflowStatus::Pending,
        }
    }

    /// Add a step to the workflow.
    pub fn add_step(&mut self, step: Step) -> Result<(), WorkflowError> {
        if self.steps.iter().any(|s| s.id == step.id) {
            return Err(WorkflowError::DuplicateStep(step.id.clone()));
        }
        self.steps.push(step);
        Ok(())
    }

    /// Validate the workflow DAG (no cycles, all dependencies exist).
    pub fn validate(&self) -> Result<(), WorkflowError> {
        let step_ids: HashSet<&str> = self.steps.iter().map(|s| s.id.as_str()).collect();

        // Check all dependencies exist
        for step in &self.steps {
            for dep in &step.depends_on {
                if !step_ids.contains(dep.as_str()) {
                    return Err(WorkflowError::DependencyNotFound(
                        dep.clone(),
                        step.id.clone(),
                    ));
                }
            }
        }

        // Check for cycles using Kahn's algorithm
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

        for step in &self.steps {
            in_degree.entry(&step.id).or_insert(0);
            for dep in &step.depends_on {
                adj.entry(dep).or_default().push(&step.id);
                *in_degree.entry(&step.id).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<&str> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut visited = 0;
        while let Some(node) = queue.pop_front() {
            visited += 1;
            if let Some(neighbors) = adj.get(node) {
                for &neighbor in neighbors {
                    let deg = in_degree.get_mut(neighbor).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        if visited != self.steps.len() {
            return Err(WorkflowError::CycleDetected);
        }

        Ok(())
    }

    /// Get steps that are ready to run (all dependencies completed).
    pub fn ready_steps(&self, completed: &HashSet<&str>) -> Vec<&Step> {
        self.steps
            .iter()
            .filter(|s| {
                // Must be pending
                !completed.contains(s.id.as_str())
                    // All dependencies must be completed
                    && s.depends_on
                        .iter()
                        .all(|dep| completed.contains(dep.as_str()))
            })
            .collect()
    }

    /// Get a step by ID.
    pub fn get_step(&self, step_id: &str) -> Option<&Step> {
        self.steps.iter().find(|s| s.id == step_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::Step;

    #[test]
    fn simple_workflow() {
        let mut wf = Workflow::new("wf1", "test");
        wf.add_step(Step::new("a", "step A", "agent")).unwrap();
        wf.add_step(Step::new("b", "step B", "agent").depends_on("a"))
            .unwrap();

        assert!(wf.validate().is_ok());
    }

    #[test]
    fn duplicate_step() {
        let mut wf = Workflow::new("wf1", "test");
        wf.add_step(Step::new("a", "step A", "agent")).unwrap();
        let result = wf.add_step(Step::new("a", "step A again", "agent"));
        assert!(matches!(result, Err(WorkflowError::DuplicateStep(_))));
    }

    #[test]
    fn missing_dependency() {
        let mut wf = Workflow::new("wf1", "test");
        wf.add_step(Step::new("b", "step B", "agent").depends_on("nonexistent"))
            .unwrap();
        assert!(matches!(
            wf.validate(),
            Err(WorkflowError::DependencyNotFound(_, _))
        ));
    }

    #[test]
    fn cycle_detection() {
        let mut wf = Workflow::new("wf1", "test");
        wf.add_step(Step::new("a", "step A", "agent").depends_on("c"))
            .unwrap();
        wf.add_step(Step::new("b", "step B", "agent").depends_on("a"))
            .unwrap();
        wf.add_step(Step::new("c", "step C", "agent").depends_on("b"))
            .unwrap();
        assert!(matches!(wf.validate(), Err(WorkflowError::CycleDetected)));
    }

    #[test]
    fn ready_steps() {
        let mut wf = Workflow::new("wf1", "test");
        wf.add_step(Step::new("a", "step A", "agent")).unwrap();
        wf.add_step(Step::new("b", "step B", "agent")).unwrap();
        wf.add_step(Step::new("c", "step C", "agent").depends_on("a"))
            .unwrap();
        wf.add_step(
            Step::new("d", "step D", "agent")
                .depends_on("a")
                .depends_on("b"),
        )
        .unwrap();

        let mut completed = HashSet::new();
        let ready = wf.ready_steps(&completed);
        assert_eq!(ready.len(), 2); // a and b have no deps

        completed.insert("a");
        let ready = wf.ready_steps(&completed);
        assert_eq!(ready.len(), 2); // b (no deps) and c (depends on a)

        completed.insert("b");
        let ready = wf.ready_steps(&completed);
        assert_eq!(ready.len(), 2); // c and d (both deps satisfied)
    }
}
