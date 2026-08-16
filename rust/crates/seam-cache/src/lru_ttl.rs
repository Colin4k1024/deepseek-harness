//! LRU/TTL cache implementation.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// A cache entry with TTL.
struct CacheEntry<V> {
    value: V,
    inserted_at: Instant,
    ttl: Duration,
}

impl<V> CacheEntry<V> {
    fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > self.ttl
    }
}

/// LRU/TTL cache — evicts least recently used entries and expired entries.
pub struct LruTtlCache<K, V> {
    entries: HashMap<K, CacheEntry<V>>,
    access_order: Vec<K>,
    max_size: usize,
    default_ttl: Duration,
}

impl<K: Eq + Hash + Clone, V: Clone> LruTtlCache<K, V> {
    /// Create a new cache with the given max size and default TTL.
    pub fn new(max_size: usize, default_ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            access_order: Vec::new(),
            max_size,
            default_ttl,
        }
    }

    /// Insert a value with the default TTL.
    pub fn insert(&mut self, key: K, value: V) {
        self.insert_with_ttl(key, value, self.default_ttl);
    }

    /// Insert a value with a custom TTL.
    pub fn insert_with_ttl(&mut self, key: K, value: V, ttl: Duration) {
        // Evict if at capacity
        if self.entries.len() >= self.max_size && !self.entries.contains_key(&key) {
            self.evict_lru();
        }

        self.entries.insert(
            key.clone(),
            CacheEntry {
                value,
                inserted_at: Instant::now(),
                ttl,
            },
        );

        // Update access order
        self.access_order.retain(|k| k != &key);
        self.access_order.push(key);
    }

    /// Get a value, refreshing its access time.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        // Check expiration
        if let Some(entry) = self.entries.get(key)
            && entry.is_expired()
        {
            self.entries.remove(key);
            self.access_order.retain(|k| k != key);
            return None;
        }

        // Update access order
        self.access_order.retain(|k| k != key);
        self.access_order.push(key.clone());

        self.entries.get(key).map(|e| &e.value)
    }

    /// Check if a key exists and is not expired.
    pub fn contains_key(&self, key: &K) -> bool {
        if let Some(entry) = self.entries.get(key) {
            !entry.is_expired()
        } else {
            false
        }
    }

    /// Remove a key.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.access_order.retain(|k| k != key);
        self.entries.remove(key).map(|e| e.value)
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_order.clear();
    }

    /// Current number of entries (including expired).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Evict expired entries.
    pub fn evict_expired(&mut self) {
        let expired: Vec<K> = self
            .entries
            .iter()
            .filter(|(_, e)| e.is_expired())
            .map(|(k, _)| k.clone())
            .collect();

        for key in expired {
            self.entries.remove(&key);
            self.access_order.retain(|k| k != &key);
        }
    }

    /// Evict the least recently used entry.
    fn evict_lru(&mut self) {
        if let Some(key) = self.access_order.first().cloned() {
            self.entries.remove(&key);
            self.access_order.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn insert_and_get() {
        let mut cache = LruTtlCache::new(10, Duration::from_secs(60));
        cache.insert("key1", "value1");
        assert_eq!(cache.get(&"key1"), Some(&"value1"));
    }

    #[test]
    fn get_missing() {
        let mut cache = LruTtlCache::<&str, &str>::new(10, Duration::from_secs(60));
        assert_eq!(cache.get(&"missing"), None);
    }

    #[test]
    fn ttl_expiration() {
        let mut cache = LruTtlCache::new(10, Duration::from_millis(0));
        cache.insert("key1", "value1");
        // TTL is 0, so entry is immediately expired
        std::thread::sleep(Duration::from_millis(1));
        assert_eq!(cache.get(&"key1"), None);
    }

    #[test]
    fn lru_eviction() {
        let mut cache = LruTtlCache::new(2, Duration::from_secs(60));
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("c", 3); // should evict "a"

        assert_eq!(cache.get(&"a"), None);
        assert_eq!(cache.get(&"b"), Some(&2));
        assert_eq!(cache.get(&"c"), Some(&3));
    }

    #[test]
    fn lru_access_refreshes_order() {
        let mut cache = LruTtlCache::new(2, Duration::from_secs(60));
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.get(&"a"); // refresh "a"
        cache.insert("c", 3); // should evict "b" (least recently used)

        assert_eq!(cache.get(&"a"), Some(&1));
        assert_eq!(cache.get(&"b"), None);
        assert_eq!(cache.get(&"c"), Some(&3));
    }

    #[test]
    fn evict_expired() {
        let mut cache = LruTtlCache::new(10, Duration::from_millis(0));
        cache.insert("a", 1);
        cache.insert("b", 2);
        std::thread::sleep(Duration::from_millis(1));

        cache.evict_expired();
        assert!(cache.is_empty());
    }

    #[test]
    fn remove() {
        let mut cache = LruTtlCache::new(10, Duration::from_secs(60));
        cache.insert("key", "value");
        assert_eq!(cache.remove(&"key"), Some("value"));
        assert!(!cache.contains_key(&"key"));
    }

    #[test]
    fn clear() {
        let mut cache = LruTtlCache::new(10, Duration::from_secs(60));
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.clear();
        assert!(cache.is_empty());
    }
}
