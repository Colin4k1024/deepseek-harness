//! L2: Subprocess capability seam.
//!
//! Defines the `Subprocess` trait and local process-tree provider.

pub mod local;
pub mod trait_def;

pub use local::LocalSubprocess;
pub use trait_def::{Subprocess, SubprocessError};
