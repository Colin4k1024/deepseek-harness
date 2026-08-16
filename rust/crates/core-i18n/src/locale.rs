//! Locale — language and region identifiers.

use serde::{Deserialize, Serialize};

/// Supported locales.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Locale {
    /// English (default)
    #[default]
    En,
    /// Chinese (Simplified)
    ZhCn,
    /// Chinese (Traditional)
    ZhTw,
    /// Japanese
    Ja,
    /// Korean
    Ko,
    /// Spanish
    Es,
    /// French
    Fr,
    /// German
    De,
    /// Portuguese
    Pt,
    /// Arabic
    Ar,
}

impl Locale {
    /// Get the locale code (e.g., "en", "zh-CN").
    pub fn code(&self) -> &'static str {
        match self {
            Self::En => "en",
            Self::ZhCn => "zh-CN",
            Self::ZhTw => "zh-TW",
            Self::Ja => "ja",
            Self::Ko => "ko",
            Self::Es => "es",
            Self::Fr => "fr",
            Self::De => "de",
            Self::Pt => "pt",
            Self::Ar => "ar",
        }
    }

    /// Parse a locale from a string code.
    pub fn from_code(code: &str) -> Option<Self> {
        match code.to_lowercase().as_str() {
            "en" | "en-us" | "en-gb" => Some(Self::En),
            "zh-cn" | "zh" | "zh-hans" => Some(Self::ZhCn),
            "zh-tw" | "zh-hant" => Some(Self::ZhTw),
            "ja" | "jp" => Some(Self::Ja),
            "ko" | "kr" => Some(Self::Ko),
            "es" => Some(Self::Es),
            "fr" => Some(Self::Fr),
            "de" => Some(Self::De),
            "pt" | "pt-br" => Some(Self::Pt),
            "ar" => Some(Self::Ar),
            _ => None,
        }
    }

    /// Get the default locale.
    pub fn default_locale() -> Self {
        Self::En
    }

    /// Get the text direction for this locale.
    pub fn direction(&self) -> TextDirection {
        match self {
            Self::Ar => TextDirection::Rtl,
            _ => TextDirection::Ltr,
        }
    }
}

impl std::fmt::Display for Locale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code())
    }
}

/// Text direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDirection {
    /// Left-to-right
    Ltr,
    /// Right-to-left
    Rtl,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_codes() {
        assert_eq!(Locale::En.code(), "en");
        assert_eq!(Locale::ZhCn.code(), "zh-CN");
        assert_eq!(Locale::Ar.code(), "ar");
    }

    #[test]
    fn parse_locale() {
        assert_eq!(Locale::from_code("en"), Some(Locale::En));
        assert_eq!(Locale::from_code("zh-CN"), Some(Locale::ZhCn));
        assert_eq!(Locale::from_code("ja"), Some(Locale::Ja));
        assert_eq!(Locale::from_code("xx"), None);
    }

    #[test]
    fn text_direction() {
        assert_eq!(Locale::En.direction(), TextDirection::Ltr);
        assert_eq!(Locale::Ar.direction(), TextDirection::Rtl);
    }
}
