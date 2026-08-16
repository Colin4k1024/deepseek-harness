//! L2: Sandbox capability seam.
//!
//! Defines `SandboxBackend` trait, `SandboxResolution` enum, and
//! platform-specific backends:
//! - **Linux**: Landlock (kernel 5.13+)
//! - **macOS**: sandbox-exec (seatbelt)
//! - **Windows**: Restricted token API (windows-rs)
//!
//! Use `resolve()` to auto-detect the best available backend for the current platform.

pub mod landlock;
pub mod resolution;
pub mod seatbelt;
pub mod trait_def;
pub mod windows;

pub use landlock::LandlockBackend;
pub use resolution::SandboxResolution;
pub use seatbelt::SeatbeltBackend;
pub use trait_def::{SandboxBackend, SandboxError, SandboxPolicy};
pub use windows::WindowsSandboxBackend;

/// Auto-detect the best available sandbox backend for the current platform.
///
/// Returns `SandboxResolution::Confined` with the platform-appropriate backend,
/// or `SandboxResolution::Unavailable` if no sandbox is available.
#[allow(unused_variables)]
pub fn resolve(policy: SandboxPolicy) -> SandboxResolution {
    if cfg!(target_os = "macos") {
        SandboxResolution::Confined(Box::new(SeatbeltBackend::new()))
    } else if cfg!(target_os = "linux") && LandlockBackend::is_supported() {
        SandboxResolution::Confined(Box::new(LandlockBackend::new()))
    } else if cfg!(target_os = "windows") {
        SandboxResolution::Confined(Box::new(WindowsSandboxBackend::new()))
    } else {
        SandboxResolution::Unavailable { requested: policy }
    }
}
