//! Caching strategy — LRU/TTL cache for LLM responses and tool results.
//!
//! Provides in-memory caching with configurable eviction policies.

pub mod lru_ttl;
pub mod strategy;

pub use lru_ttl::LruTtlCache;
pub use strategy::{CacheConfig, CacheKey, CacheStrategy};
