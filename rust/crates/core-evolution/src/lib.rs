//! L0/L1: Evolution primitives — Gene and Capsule.
//!
//! Inspired by the Oris agent runtime's state management patterns:
//! - **Gene**: A strategy or policy DNA that defines agent behavior
//! - **Capsule**: A verified experience snapshot that can be replayed
//! - **EvolutionEvent**: Append-only event log for tracking agent evolution

pub mod capsule;
pub mod event;
pub mod gene;

pub use capsule::Capsule;
pub use event::EvolutionEvent;
pub use gene::Gene;
