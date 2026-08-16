//! Mobile adaptation — responsive API design and mobile SDK support.
//!
//! Provides mobile-optimized API responses, adaptive content delivery,
//! and utilities for building mobile SDKs.

pub mod adaptive;
pub mod client;

pub use adaptive::{AdaptiveConfig, ContentAdapter};
pub use client::{MobileClient, MobileClientConfig};
