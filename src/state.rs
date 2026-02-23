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
