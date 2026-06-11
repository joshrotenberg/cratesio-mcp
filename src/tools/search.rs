//! Search crates tool

use std::sync::Arc;

use crate::client::{CratesQuery, Sort};
use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, CrateSummary, format_number};

/// Input for searching crates
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchInput {
    /// Search query (crate name or keywords)
    query: String,
    /// Sort order: relevance, downloads, recent-downloads, recent-updates, new
    #[serde(default = "default_sort")]
    sort: String,
}

fn default_sort() -> String {
    "relevance".to_string()
}

fn parse_sort(s: &str) -> Sort {
    match s {
        "downloads" => Sort::Downloads,
        "recent-downloads" => Sort::RecentDownloads,
        "recent-updates" => Sort::RecentUpdates,
        "new" => Sort::NewlyAdded,
        _ => Sort::Relevance,
    }
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("search_crates")
        .title("Search Crates")
        .description(
            "Search for Rust crates on crates.io. Returns crate names, descriptions, \
             download counts, and repository links.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<SearchInput>| async move {
                let sort = parse_sort(&input.sort);
                let query = CratesQuery::builder()
                    .search(&input.query)
                    .sort(sort)
                    .build();

                let response = state
                    .client
                    .crates(query)
                    .await
                    .tool_context("Crates.io API error")?;

                // Save search for resources
                let summaries: Vec<_> = response
                    .crates
                    .iter()
                    .map(|c| CrateSummary {
                        name: c.name.clone(),
                        description: c.description.clone(),
                        max_version: c.max_version.clone(),
                        downloads: c.downloads,
                    })
                    .collect();
                state.save_search(input.query.clone(), summaries).await;

                // Format results
                let mut output = format!(
                    "Found {} crates matching '{}' (showing {}):\n\n",
                    response.meta.total,
                    input.query,
                    response.crates.len()
                );

                for (i, c) in response.crates.iter().enumerate() {
                    output.push_str(&format!("{}. **{}** v{}\n", i + 1, c.name, c.max_version));
                    if let Some(desc) = &c.description {
                        output.push_str(&format!("   {}\n", desc.trim()));
                    }
                    output.push_str(&format!(
                        "   Downloads: {} | Recent: {}\n",
                        format_number(c.downloads),
                        c.recent_downloads.map(format_number).unwrap_or_default()
                    ));
                    if let Some(repo) = &c.repository {
                        output.push_str(&format!("   Repo: {}\n", repo));
                    }
                    output.push('\n');
                }

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::sync::RwLock;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::client::CratesIoClient;
    use crate::client::docsrs::DocsRsClient;
    use crate::client::osv::OsvClient;
    use crate::docs::cache::DocsCache;
    use crate::state::AppState;

    fn test_state(base_url: &str) -> Arc<AppState> {
        Arc::new(AppState {
            client: CratesIoClient::with_base_url(
                "test",
                Duration::from_millis(0),
                Duration::from_secs(30),
                base_url,
            )
            .unwrap(),
            docsrs_client: DocsRsClient::with_base_url("test", Duration::from_secs(30), base_url)
                .unwrap(),
            osv_client: OsvClient::with_base_url(
                "test",
                Duration::from_secs(30),
                "http://localhost:1",
            )
            .unwrap(),
            docs_cache: DocsCache::new(10, Duration::from_secs(3600)),
            recent_searches: RwLock::new(Vec::new()),
        })
    }

    #[tokio::test]
    async fn search_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"query": "nonexistent-xyz"}))
            .await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn search_empty_results() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crates": [],
                "meta": {"total": 0}
            })))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"query": "zzz-no-match"}))
            .await;
        assert!(result.all_text().contains("Found 0 crates"));
    }
}
