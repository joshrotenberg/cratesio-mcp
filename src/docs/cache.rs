//! In-memory LRU cache for parsed rustdoc JSON.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustdoc_types::Crate;
use tokio::sync::RwLock;

use crate::client::docsrs::{DocsRsClient, DocsRsError};

struct CacheEntry {
    krate: Arc<Crate>,
    fetched_at: Instant,
    /// Tracks last access for LRU eviction.
    last_accessed: Instant,
}

/// Cache for parsed `rustdoc_types::Crate` values.
///
/// Keyed by `(crate_name, version)`. Supports TTL expiration and LRU eviction.
pub struct DocsCache {
    entries: RwLock<HashMap<(String, String), CacheEntry>>,
    max_entries: usize,
    ttl: Duration,
}

impl DocsCache {
    /// Create a new cache with the given capacity and TTL.
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            max_entries,
            ttl,
        }
    }

    /// Get a cached crate, if present and not expired.
    pub async fn get(&self, name: &str, version: &str) -> Option<Arc<Crate>> {
        let key = (name.to_string(), version.to_string());
        let mut entries = self.entries.write().await;
        let entry = entries.get_mut(&key)?;
        if entry.fetched_at.elapsed() > self.ttl {
            entries.remove(&key);
            return None;
        }
        entry.last_accessed = Instant::now();
        Some(Arc::clone(&entry.krate))
    }

    /// Insert a crate into the cache, evicting LRU if full.
    pub async fn insert(&self, name: &str, version: &str, krate: Arc<Crate>) {
        let key = (name.to_string(), version.to_string());
        let mut entries = self.entries.write().await;

        // Evict expired entries first
        entries.retain(|_, v| v.fetched_at.elapsed() <= self.ttl);

        // LRU eviction if still at capacity
        if entries.len() >= self.max_entries
            && !entries.contains_key(&key)
            && let Some(lru_key) = entries
                .iter()
                .min_by_key(|(_, v)| v.last_accessed)
                .map(|(k, _)| k.clone())
        {
            entries.remove(&lru_key);
        }

        let now = Instant::now();
        entries.insert(
            key,
            CacheEntry {
                krate,
                fetched_at: now,
                last_accessed: now,
            },
        );
    }

    /// Get a cached crate, or fetch and cache it on miss.
    pub async fn get_or_fetch(
        &self,
        client: &DocsRsClient,
        name: &str,
        version: &str,
    ) -> Result<Arc<Crate>, DocsRsError> {
        if let Some(krate) = self.get(name, version).await {
            return Ok(krate);
        }

        let krate = client.fetch_rustdoc(name, version).await?;
        let krate = Arc::new(krate);
        self.insert(name, version, Arc::clone(&krate)).await;
        Ok(krate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_crate() -> Crate {
        let json = serde_json::json!({
            "root": 0,
            "crate_version": "1.0.0",
            "includes_private": false,
            "index": {},
            "paths": {},
            "external_crates": {},
            "target": {
                "triple": "x86_64-unknown-linux-gnu",
                "target_features": []
            },
            "format_version": 39
        });
        serde_json::from_value(json).unwrap()
    }

    #[tokio::test]
    async fn insert_and_get() {
        let cache = DocsCache::new(10, Duration::from_secs(3600));
        let krate = Arc::new(synthetic_crate());
        cache.insert("serde", "1.0.0", Arc::clone(&krate)).await;
        let cached = cache.get("serde", "1.0.0").await;
        assert!(cached.is_some());
    }

    #[tokio::test]
    async fn miss_returns_none() {
        let cache = DocsCache::new(10, Duration::from_secs(3600));
        assert!(cache.get("nonexistent", "1.0.0").await.is_none());
    }

    #[tokio::test]
    async fn ttl_expiration() {
        let cache = DocsCache::new(10, Duration::from_millis(1));
        let krate = Arc::new(synthetic_crate());
        cache.insert("serde", "1.0.0", krate).await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(cache.get("serde", "1.0.0").await.is_none());
    }

    #[tokio::test]
    async fn lru_eviction() {
        let cache = DocsCache::new(2, Duration::from_secs(3600));
        let k1 = Arc::new(synthetic_crate());
        let k2 = Arc::new(synthetic_crate());
        let k3 = Arc::new(synthetic_crate());

        cache.insert("a", "1.0.0", k1).await;
        cache.insert("b", "1.0.0", k2).await;
        // Access "a" so "b" becomes LRU
        cache.get("a", "1.0.0").await;
        // Insert "c" -- should evict "b"
        cache.insert("c", "1.0.0", k3).await;

        assert!(cache.get("a", "1.0.0").await.is_some());
        assert!(cache.get("b", "1.0.0").await.is_none());
        assert!(cache.get("c", "1.0.0").await.is_some());
    }
}
