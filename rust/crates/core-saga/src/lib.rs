//! Saga pattern — distributed transactions with compensation.
//!
//! A Saga is a sequence of local transactions. Each transaction has a
//! compensating action. If any step fails, the compensations are executed
//! in reverse order to undo the effects.

pub mod saga;
pub mod step;

pub use saga::{Saga, SagaError, SagaResult, SagaStatus};
pub use step::{SagaStep, StepResult};
