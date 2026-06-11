//! Get downloads tool

use std::collections::HashMap;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, format_number};

/// Input for getting downloads data
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadsInput {
    /// Crate name
    name: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_downloads")
        .title("Get Downloads")
        .description(
            "Get download statistics for a crate including total downloads \
             and recent download trends.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<DownloadsInput>| async move {
                let response = state
                    .client
                    .crate_downloads(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                let mut output = format!("# {} - Download Statistics\n\n", input.name);

                // Get recent downloads (last 90 days from version_downloads)
                let total: u64 = response.version_downloads.iter().map(|v| v.downloads).sum();
                output.push_str(&format!(
                    "**Recent downloads (90 days):** {}\n\n",
                    format_number(total)
                ));

                // Build version ID -> version string map
                let version_names: HashMap<u64, &str> = response
                    .versions
                    .iter()
                    .map(|v| (v.id, v.num.as_str()))
                    .collect();

                // Show per-version breakdown
                output.push_str("## By Version\n\n");
                let mut version_totals: HashMap<u64, u64> = HashMap::new();
                for vd in &response.version_downloads {
                    *version_totals.entry(vd.version).or_default() += vd.downloads;
                }

                let mut versions: Vec<_> = version_totals.iter().collect();
                versions.sort_by(|a, b| b.1.cmp(a.1));

                for (version_id, downloads) in versions.iter().take(10) {
                    let name = version_names.get(version_id).copied().unwrap_or("unknown");
                    output.push_str(&format!("- v{}: {}\n", name, format_number(**downloads)));
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

    // No meaningful empty case: empty version_downloads just renders a 0-total header with no rows.

    #[tokio::test]
    async fn downloads_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/nonexistent/downloads"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "nonexistent"})).await;

        assert!(result.is_error);
    }

    #[tokio::test]
    async fn downloads_api_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/my-crate/downloads"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "my-crate"})).await;

        assert!(result.is_error);
    }
}
