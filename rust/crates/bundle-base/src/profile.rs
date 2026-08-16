//! Profile and configuration for L3 Composition.
//!
//! A `Profile` is a runtime configuration that specifies which LLM provider,
//! sandbox backend, persistence layer, and other options are active.

use serde::{Deserialize, Serialize};

/// LLM provider selection.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    #[default]
    DeepSeek,
    OpenAi,
    Anthropic,
    Custom(String),
}

/// Sandbox backend selection.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SandboxBackend {
    #[default]
    Auto,
    Landlock,
    Seatbelt,
    None,
}

/// Persistence backend selection.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PersistenceBackend {
    #[default]
    Jsonl,
    Sqlite,
    None,
}

/// Runtime profile — configures which components are active.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Profile name (e.g., "default", "production", "development").
    pub name: String,

    /// LLM provider to use.
    pub llm: LlmProvider,

    /// LLM model name.
    pub model: String,

    /// Sandbox backend.
    pub sandbox: SandboxBackend,

    /// Persistence backend.
    pub persistence: PersistenceBackend,

    /// Path for persistence storage (directory for JSONL, file for SQLite).
    pub persistence_path: Option<String>,

    /// Whether to enable MCP tool discovery.
    pub mcp: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "default".into(),
            llm: LlmProvider::default(),
            model: "deepseek-chat".into(),
            sandbox: SandboxBackend::Auto,
            persistence: PersistenceBackend::default(),
            persistence_path: None,
            mcp: false,
        }
    }
}

impl Profile {
    /// Create a development profile (JSONL persistence, no sandbox).
    pub fn development() -> Self {
        Self {
            name: "development".into(),
            sandbox: SandboxBackend::None,
            persistence: PersistenceBackend::Jsonl,
            persistence_path: Some(".dsh/sessions".into()),
            ..Default::default()
        }
    }

    /// Create a production profile (SQLite persistence, auto sandbox).
    pub fn production() -> Self {
        Self {
            name: "production".into(),
            sandbox: SandboxBackend::Auto,
            persistence: PersistenceBackend::Sqlite,
            persistence_path: Some("sessions.db".into()),
            ..Default::default()
        }
    }

    /// Load a profile from a JSON file.
    pub fn from_json_file(path: &str) -> Result<Self, serde_json::Error> {
        let content = std::fs::read_to_string(path).map_err(serde_json::Error::io)?;
        serde_json::from_str(&content)
    }

    /// Save the profile to a JSON file.
    pub fn to_json_file(&self, path: &str) -> Result<(), serde_json::Error> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content).map_err(serde_json::Error::io)
    }

    /// Check if a feature is available at compile time.
    pub fn feature_available(feature: &str) -> bool {
        match feature {
            "llm-deepseek" => cfg!(feature = "llm-deepseek"),
            "llm-openai" => cfg!(feature = "llm-openai"),
            "llm-anthropic" => cfg!(feature = "llm-anthropic"),
            "sandbox" => cfg!(feature = "sandbox"),
            "persistence-jsonl" => cfg!(feature = "persistence-jsonl"),
            "persistence-sqlite" => cfg!(feature = "persistence-sqlite"),
            "mcp" => cfg!(feature = "mcp"),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile() {
        let profile = Profile::default();
        assert_eq!(profile.name, "default");
        assert_eq!(profile.llm, LlmProvider::DeepSeek);
        assert_eq!(profile.sandbox, SandboxBackend::Auto);
    }

    #[test]
    fn development_profile() {
        let profile = Profile::development();
        assert_eq!(profile.name, "development");
        assert_eq!(profile.sandbox, SandboxBackend::None);
        assert_eq!(profile.persistence, PersistenceBackend::Jsonl);
    }

    #[test]
    fn production_profile() {
        let profile = Profile::production();
        assert_eq!(profile.name, "production");
        assert_eq!(profile.persistence, PersistenceBackend::Sqlite);
    }

    #[test]
    fn profile_roundtrip_json() {
        let profile = Profile::development();
        let json = serde_json::to_string(&profile).unwrap();
        let loaded: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.name, profile.name);
        assert_eq!(loaded.llm, profile.llm);
    }

    #[test]
    fn feature_check() {
        // These should work regardless of which features are enabled
        let _ = Profile::feature_available("llm-deepseek");
        let _ = Profile::feature_available("sandbox");
        assert!(!Profile::feature_available("nonexistent"));
    }
}
