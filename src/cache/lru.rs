//! LRU cache backend with per-entry TTL.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use lru::LruCache as RawLru;

use super::Cache;

const DEFAULT_CAPACITY: usize = 128;

type Entry = (Instant, serde_json::Value);

/// Thread-safe LRU cache with per-entry TTL.
pub struct LruCache {
    inner: Mutex<RawLru<String, Entry>>,
    ttl: Duration,
}

impl LruCache {
    /// Create with given capacity and TTL.
    ///
    /// # Panics
    ///
    /// Never panics — `capacity.max(1)` is always non-zero.
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        let cap = std::num::NonZeroUsize::new(capacity.max(1)).expect("capacity >= 1");
        Self {
            inner: Mutex::new(RawLru::new(cap)),
            ttl,
        }
    }

    /// Create with default capacity (128) and given TTL.
    pub fn with_ttl(ttl: Duration) -> Self {
        Self::new(DEFAULT_CAPACITY, ttl)
    }
}

impl Cache for LruCache {
    fn get(&self, key: &str) -> Option<serde_json::Value> {
        let mut guard = self.inner.lock().expect("cache lock");
        match guard.get(key) {
            Some((inserted_at, value)) if inserted_at.elapsed() < self.ttl => {
                Some(value.clone())
            }
            Some(_) => {
                // Expired — evict and treat as miss.
                guard.pop(key);
                None
            }
            None => None,
        }
    }

    fn put(&self, key: String, value: serde_json::Value) {
        let mut guard = self.inner.lock().expect("cache lock");
        guard.put(key, (Instant::now(), value));
    }

    fn invalidate_prefix(&self, prefix: &str) {
        let mut guard = self.inner.lock().expect("cache lock");
        // Collect keys matching the prefix, then remove them.
        // Linear scan is acceptable at 128 entries.
        let to_remove: Vec<String> = guard
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, _)| k.clone())
            .collect();
        for k in to_remove {
            guard.pop(&k);
        }
    }
}
