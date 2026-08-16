//! WCAG compliance validation.

use serde::{Deserialize, Serialize};

/// WCAG compliance level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WcagLevel {
    /// Level A — minimum accessibility.
    A,
    /// Level AA — standard accessibility (recommended).
    Aa,
    /// Level AAA — enhanced accessibility.
    Aaa,
}

/// WCAG validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WcagReport {
    /// Compliance level checked.
    pub level: WcagLevel,

    /// Whether the response passes.
    pub passes: bool,

    /// List of issues found.
    pub issues: Vec<WcagIssue>,
}

/// A WCAG compliance issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WcagIssue {
    /// WCAG criterion (e.g., "1.1.1").
    pub criterion: String,

    /// Issue description.
    pub description: String,

    /// Severity.
    pub severity: IssueSeverity,
}

/// Issue severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueSeverity {
    Error,
    Warning,
    Info,
}

/// WCAG validator — checks API responses for accessibility compliance.
pub struct WcagValidator;

impl WcagValidator {
    /// Validate a response for WCAG AA compliance.
    pub fn validate_aa(content: &str) -> WcagReport {
        let mut issues = Vec::new();

        // Check for text content
        if content.is_empty() {
            issues.push(WcagIssue {
                criterion: "1.1.1".into(),
                description: "Empty content — screen readers have nothing to read".into(),
                severity: IssueSeverity::Error,
            });
        }

        // Check for very long content without structure
        if content.len() > 5000 && !content.contains('\n') {
            issues.push(WcagIssue {
                criterion: "1.3.1".into(),
                description:
                    "Very long content without line breaks — hard for screen readers to navigate"
                        .into(),
                severity: IssueSeverity::Warning,
            });
        }

        // Check for special characters that might confuse screen readers
        if content.contains("🔥") || content.contains("💀") || content.contains("⚡") {
            issues.push(WcagIssue {
                criterion: "1.1.1".into(),
                description: "Decorative emoji without text alternative".into(),
                severity: IssueSeverity::Info,
            });
        }

        WcagReport {
            level: WcagLevel::Aa,
            passes: issues.iter().all(|i| i.severity != IssueSeverity::Error),
            issues,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_content_fails() {
        let report = WcagValidator::validate_aa("");
        assert!(!report.passes);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].criterion, "1.1.1");
    }

    #[test]
    fn normal_content_passes() {
        let report = WcagValidator::validate_aa("Hello, world!");
        assert!(report.passes);
        assert!(report.issues.is_empty());
    }

    #[test]
    fn long_content_warns() {
        let long_content = "a".repeat(6000);
        let report = WcagValidator::validate_aa(&long_content);
        assert!(report.passes); // Warning, not error
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].severity, IssueSeverity::Warning);
    }

    #[test]
    fn emoji_info() {
        let report = WcagValidator::validate_aa("Hello 🔥");
        assert!(report.passes);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].severity, IssueSeverity::Info);
    }
}
