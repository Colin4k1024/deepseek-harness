//! Accessibility (a11y) — accessible API design patterns.
//!
//! Provides accessible API response formatting, screen reader support,
//! and WCAG compliance utilities for the web surface.

pub mod response;
pub mod wcag;

pub use response::{AccessibleResponse, ResponseFormat};
pub use wcag::{WcagLevel, WcagValidator};
