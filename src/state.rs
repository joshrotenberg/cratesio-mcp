//! Shared application state

use std::time::Duration;

use crate::client::CratesIoClient;
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
    /// Recent search queries (exposed as a resource)
    pub recent_searches: RwLock<Vec<(String, Vec<CrateSummary>)>>,
}

impl AppState {
    /// Create new application state
    ///
    /// # Arguments
    /// * `rate_limit` - Minimum interval between crates.io API calls
    pub fn new(rate_limit: Duration) -> Result<Self, tower_mcp::BoxError> {
        let client = CratesIoClient::new(
            "cratesio-mcp (https://github.com/joshrotenberg/cratesio-mcp)",
            rate_limit,
        )
        .map_err(|e| format!("Failed to create crates.io client: {e}"))?;

        Ok(Self {
            client,
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
