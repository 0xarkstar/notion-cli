//! GET response cache — default OFF; opt-in via
//! `NOTION_CLI_CACHE_TTL` env. Writes invalidate entries for the
//! mutated entity id.

pub mod lru;

use std::sync::Arc;

pub use lru::LruCache;

/// Cache trait — agnostic to backend. LRU is the default.
pub trait Cache: Send + Sync {
    fn get(&self, key: &str) -> Option<serde_json::Value>;
    fn put(&self, key: String, value: serde_json::Value);
    /// Invalidate any keys whose path starts with the given prefix.
    /// Called after writes so stale GETs on the same entity don't
    /// leak back.
    fn invalidate_prefix(&self, prefix: &str);
}

pub type SharedCache = Arc<dyn Cache>;
