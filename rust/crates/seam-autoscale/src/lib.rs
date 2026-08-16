//! Auto-scaling — scale instances based on metrics.

pub mod policy;
pub mod scaler;

pub use policy::{ScaleDirection, ScalePolicy};
pub use scaler::{AutoScaler, ScaleEvent};
