//! Shared application state

use std::time::Duration;

use crate::client::CratesIoClient;
use crate::client::docsrs::DocsRsClient;
use crate::client::osv::OsvClient;
use crate::docs::cache::DocsCache;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

/// Summary of a crate from search results (for resource storage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateSummary {
    pub name: String,
    pub description: Option<String>,
    pub max_version: String,
    pub downloads: u64,
}

/// Shared state for the MCP server
pub struct AppState {
    /// Crates.io API client (already rate-limited internally)
    pub client: CratesIoClient,
    /// docs.rs API client for rustdoc JSON
    pub docsrs_client: DocsRsClient,
    /// OSV.dev API client for vulnerability lookups
    pub osv_client: OsvClient,
    /// Cache for parsed rustdoc JSON
    pub docs_cache: DocsCache,
    /// Recent search queries (exposed as a resource)
    pub recent_searches: RwLock<Vec<(String, Vec<CrateSummary>)>>,
}

impl AppState {
    /// Create new application state
    ///
    /// # Arguments
    /// * `rate_limit` - Minimum interval between crates.io API calls
    /// * `docs_cache_max_entries` - Maximum cached rustdoc JSON entries
    /// * `docs_cache_ttl` - TTL for cached rustdoc JSON entries
    pub fn new(
        rate_limit: Duration,
        docs_cache_max_entries: usize,
        docs_cache_ttl: Duration,
    ) -> Result<Self, tower_mcp::BoxError> {
        let user_agent = "cratesio-mcp (https://github.com/joshrotenberg/cratesio-mcp)";
        let client = CratesIoClient::new(user_agent, rate_limit)
            .map_err(|e| format!("Failed to create crates.io client: {e}"))?;
        let docsrs_client = DocsRsClient::new(user_agent)
            .map_err(|e| format!("Failed to create docs.rs client: {e}"))?;
        let osv_client =
            OsvClient::new(user_agent).map_err(|e| format!("Failed to create OSV client: {e}"))?;
        let docs_cache = DocsCache::new(docs_cache_max_entries, docs_cache_ttl);

        Ok(Self {
            client,
            docsrs_client,
            osv_client,
            docs_cache,
            recent_searches: RwLock::new(Vec::new()),
        })
    }

    /// Create application state with a custom crates.io base URL (for testing).
    ///
    /// Points the `CratesIoClient` at the given base URL (e.g. wiremock server)
    /// with zero rate limiting for fast test execution. DocsRs/OSV clients use
    /// default constructors.
    pub fn with_base_url(base_url: &str) -> Result<Self, tower_mcp::BoxError> {
        let user_agent = "cratesio-mcp-test";
        let client = CratesIoClient::with_base_url(user_agent, Duration::from_millis(0), base_url)
            .map_err(|e| format!("Failed to create crates.io client: {e}"))?;
        let docsrs_client = DocsRsClient::new(user_agent)
            .map_err(|e| format!("Failed to create docs.rs client: {e}"))?;
        let osv_client =
            OsvClient::new(user_agent).map_err(|e| format!("Failed to create OSV client: {e}"))?;
        let docs_cache = DocsCache::new(10, Duration::from_secs(60));

        Ok(Self {
            client,
            docsrs_client,
            osv_client,
            docs_cache,
            recent_searches: RwLock::new(Vec::new()),
        })
    }

    /// Create application state with custom base URLs for all clients (for testing).
    ///
    /// Points all three clients at specified base URLs with zero rate limiting.
    pub fn with_all_base_urls(
        crates_url: &str,
        docsrs_url: &str,
        osv_url: &str,
    ) -> Result<Self, tower_mcp::BoxError> {
        let user_agent = "cratesio-mcp-test";
        let client =
            CratesIoClient::with_base_url(user_agent, Duration::from_millis(0), crates_url)
                .map_err(|e| format!("Failed to create crates.io client: {e}"))?;
        let docsrs_client = DocsRsClient::with_base_url(user_agent, docsrs_url)
            .map_err(|e| format!("Failed to create docs.rs client: {e}"))?;
        let osv_client = OsvClient::with_base_url(user_agent, osv_url)
            .map_err(|e| format!("Failed to create OSV client: {e}"))?;
        let docs_cache = DocsCache::new(10, Duration::from_secs(60));

        Ok(Self {
            client,
            docsrs_client,
            osv_client,
            docs_cache,
            recent_searches: RwLock::new(Vec::new()),
        })
    }

    /// Save a search query and its results for the recent searches resource
    pub async fn save_search(&self, query: String, results: Vec<CrateSummary>) {
        let mut searches = self.recent_searches.write().await;
        // Keep only last 10 searches
        if searches.len() >= 10 {
            searches.remove(0);
        }
        searches.push((query, results));
    }
}

/// Helper to format large numbers in a human-readable way
pub fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_number_small() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(1), "1");
        assert_eq!(format_number(999), "999");
    }

    #[test]
    fn format_number_thousands() {
        assert_eq!(format_number(1_000), "1.0K");
        assert_eq!(format_number(1_500), "1.5K");
        assert_eq!(format_number(999_999), "1000.0K");
    }

    #[test]
    fn format_number_millions() {
        assert_eq!(format_number(1_000_000), "1.0M");
        assert_eq!(format_number(2_500_000), "2.5M");
        assert_eq!(format_number(50_000_000_000), "50000.0M");
    }

    #[tokio::test]
    async fn save_search_stores_entry() {
        let state = AppState::with_base_url("http://unused").unwrap();

        state
            .save_search(
                "tokio".to_string(),
                vec![CrateSummary {
                    name: "tokio".to_string(),
                    description: Some("Async runtime".to_string()),
                    max_version: "1.0.0".to_string(),
                    downloads: 100,
                }],
            )
            .await;

        let searches = state.recent_searches.read().await;
        assert_eq!(searches.len(), 1);
        assert_eq!(searches[0].0, "tokio");
        assert_eq!(searches[0].1[0].name, "tokio");
    }

    #[tokio::test]
    async fn save_search_caps_at_10() {
        let state = AppState::with_base_url("http://unused").unwrap();

        for i in 0..12 {
            state.save_search(format!("query-{i}"), Vec::new()).await;
        }

        let searches = state.recent_searches.read().await;
        assert_eq!(searches.len(), 10);
        // Oldest entries should have been evicted
        assert_eq!(searches[0].0, "query-2");
        assert_eq!(searches[9].0, "query-11");
    }
}
