//! Workflow engine — DAG-based multi-step agent workflows.
//!
//! Defines workflows as directed acyclic graphs (DAGs) of steps.
//! The executor runs steps respecting dependencies, parallelizing
//! independent steps automatically.

pub mod executor;
pub mod step;
pub mod workflow;

pub use executor::{ExecutionResult, StepExecutor, WorkflowExecutor};
pub use step::{Step, StepId, StepResult, StepStatus};
pub use workflow::{Workflow, WorkflowError, WorkflowStatus};
