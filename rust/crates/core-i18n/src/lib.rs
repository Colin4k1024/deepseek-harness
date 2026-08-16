//! Internationalization (i18n) — locale-aware messages and API responses.
//!
//! Provides a translation system for error messages, API responses,
//! and documentation. Supports multiple locales with fallback to English.

pub mod locale;
pub mod message;
pub mod translator;

pub use locale::Locale;
pub use message::{Message, MessageId};
pub use translator::Translator;
