//! Cross-modal retrieval — content types and cross-modality search.

pub mod content;
pub mod retrieval;

pub use content::{ContentMetadata, Modality, MultimodalContent};
pub use retrieval::{CrossModalIndex, CrossModalResult};
