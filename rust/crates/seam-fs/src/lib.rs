//! L2: Filesystem capability seam.
//!
//! Defines the `FileSystem` trait and local filesystem provider.

pub mod local;
pub mod trait_def;

pub use local::LocalFileSystem;
pub use trait_def::{FileSystem, FsError};
