//! Cache strategy — configurable caching for LLM responses and tool results.

use serde::{Deserialize, Serialize};

/// Cache key types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CacheKey {
    /// Cache by message content hash.
    Content(String),
    /// Cache by tool name + arguments hash.
    ToolCall { name: String, args_hash: String },
    /// Cache by session ID + turn number.
    Session { session_id: String, turn: u64 },
}

/// Cache configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Maximum number of entries.
    pub max_entries: usize,

    /// Default TTL in seconds.
    pub ttl_secs: u64,

    /// Whether to cache LLM responses.
    pub cache_llm: bool,

    /// Whether to cache tool results.
    pub cache_tools: bool,

    /// Cache key strategy.
    pub key_strategy: CacheKeyStrategy,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            ttl_secs: 300, // 5 minutes
            cache_llm: true,
            cache_tools: true,
            key_strategy: CacheKeyStrategy::ContentHash,
        }
    }
}

/// Cache key strategy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheKeyStrategy {
    /// Hash the full content.
    #[default]
    ContentHash,
    /// Use the first N characters.
    Prefix(usize),
    /// Use semantic similarity (future).
    Semantic,
}

/// Cache strategy — determines what to cache and how.
pub struct CacheStrategy {
    config: CacheConfig,
}

impl CacheStrategy {
    /// Create a new cache strategy.
    pub fn new(config: CacheConfig) -> Self {
        Self { config }
    }

    /// Check if an LLM response should be cached.
    pub fn should_cache_llm(&self) -> bool {
        self.config.cache_llm
    }

    /// Check if a tool result should be cached.
    pub fn should_cache_tool(&self) -> bool {
        self.config.cache_tools
    }

    /// Get the max entries.
    pub fn max_entries(&self) -> usize {
        self.config.max_entries
    }

    /// Get the TTL duration.
    pub fn ttl(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.config.ttl_secs)
    }

    /// Generate a cache key for content.
    pub fn content_key(&self, content: &str) -> CacheKey {
        match self.config.key_strategy {
            CacheKeyStrategy::ContentHash => CacheKey::Content(format!("{:x}", md5_hash(content))),
            CacheKeyStrategy::Prefix(n) => {
                let prefix: String = content.chars().take(n).collect();
                CacheKey::Content(prefix)
            }
            CacheKeyStrategy::Semantic => {
                // Future: use embedding similarity
                CacheKey::Content(content.to_string())
            }
        }
    }
}

/// Simple MD5-like hash (using built-in hasher for now).
fn md5_hash(content: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = CacheConfig::default();
        assert_eq!(config.max_entries, 1000);
        assert_eq!(config.ttl_secs, 300);
        assert!(config.cache_llm);
        assert!(config.cache_tools);
    }

    #[test]
    fn content_key_deterministic() {
        let strategy = CacheStrategy::new(CacheConfig::default());
        let k1 = strategy.content_key("hello world");
        let k2 = strategy.content_key("hello world");
        assert_eq!(k1, k2);
    }

    #[test]
    fn content_key_different() {
        let strategy = CacheStrategy::new(CacheConfig::default());
        let k1 = strategy.content_key("hello");
        let k2 = strategy.content_key("world");
        assert_ne!(k1, k2);
    }
}
