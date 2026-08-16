//! Accessible response formatting for API responses.

use serde::{Deserialize, Serialize};

/// Response format options for accessibility.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponseFormat {
    /// Standard JSON response.
    #[default]
    Json,
    /// Plain text response (for screen readers).
    PlainText,
    /// HTML with ARIA attributes.
    Html,
    /// Simplified response (reduced complexity).
    Simplified,
}

/// An accessible API response with metadata for screen readers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibleResponse {
    /// The actual response content.
    pub content: String,

    /// Response format.
    pub format: ResponseFormat,

    /// Accessible label for screen readers.
    pub aria_label: Option<String>,

    /// Role for ARIA.
    pub aria_role: Option<String>,

    /// Whether this is a live region (updates dynamically).
    pub aria_live: bool,

    /// Language of the content.
    pub lang: String,

    /// Text direction.
    pub direction: String,
}

impl AccessibleResponse {
    /// Create a new accessible response.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            format: ResponseFormat::Json,
            aria_label: None,
            aria_role: None,
            aria_live: false,
            lang: "en".into(),
            direction: "ltr".into(),
        }
    }

    /// Set the format.
    pub fn with_format(mut self, format: ResponseFormat) -> Self {
        self.format = format;
        self
    }

    /// Set the ARIA label.
    pub fn with_aria_label(mut self, label: impl Into<String>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// Set the ARIA role.
    pub fn with_aria_role(mut self, role: impl Into<String>) -> Self {
        self.aria_role = Some(role.into());
        self
    }

    /// Mark as live region.
    pub fn with_live(mut self) -> Self {
        self.aria_live = true;
        self
    }

    /// Set the language.
    pub fn with_lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = lang.into();
        self
    }

    /// Set the text direction.
    pub fn with_direction(mut self, direction: impl Into<String>) -> Self {
        self.direction = direction.into();
        self
    }

    /// Convert to plain text (for screen readers).
    pub fn to_plain_text(&self) -> String {
        self.content.clone()
    }

    /// Convert to HTML with ARIA attributes.
    pub fn to_html(&self) -> String {
        let mut html = String::new();
        html.push_str(&format!(
            "<div lang=\"{}\" dir=\"{}\"",
            self.lang, self.direction
        ));

        if let Some(role) = &self.aria_role {
            html.push_str(&format!(" role=\"{role}\""));
        }
        if let Some(label) = &self.aria_label {
            html.push_str(&format!(" aria-label=\"{label}\""));
        }
        if self.aria_live {
            html.push_str(" aria-live=\"polite\"");
        }

        html.push('>');
        html.push_str(&self.content);
        html.push_str("</div>");
        html
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_response() {
        let resp = AccessibleResponse::new("Hello");
        assert_eq!(resp.content, "Hello");
        assert_eq!(resp.format, ResponseFormat::Json);
        assert_eq!(resp.lang, "en");
        assert_eq!(resp.direction, "ltr");
    }

    #[test]
    fn to_plain_text() {
        let resp = AccessibleResponse::new("Hello world");
        assert_eq!(resp.to_plain_text(), "Hello world");
    }

    #[test]
    fn to_html() {
        let resp = AccessibleResponse::new("Hello")
            .with_aria_role("status")
            .with_aria_label("Greeting")
            .with_live()
            .with_lang("en");

        let html = resp.to_html();
        assert!(html.contains("lang=\"en\""));
        assert!(html.contains("role=\"status\""));
        assert!(html.contains("aria-label=\"Greeting\""));
        assert!(html.contains("aria-live=\"polite\""));
        assert!(html.contains("Hello"));
    }

    #[test]
    fn to_html_rtl() {
        let resp = AccessibleResponse::new("مرحبا")
            .with_lang("ar")
            .with_direction("rtl");

        let html = resp.to_html();
        assert!(html.contains("dir=\"rtl\""));
        assert!(html.contains("lang=\"ar\""));
    }
}
