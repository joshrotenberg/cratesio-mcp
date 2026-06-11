//! Get crate versions tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, format_number};

/// Input for getting versions
#[derive(Debug, Deserialize, JsonSchema)]
pub struct VersionsInput {
    /// Crate name
    name: String,
    /// Include yanked versions
    #[serde(default)]
    include_yanked: bool,
    /// Maximum number of versions (default: 10)
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    10
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_crate_versions")
        .title("Get Crate Versions")
        .description(
            "Get version history for a crate including version numbers, release dates, \
             download counts, and yanked status.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<VersionsInput>| async move {
                let response = state
                    .client
                    .get_crate(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                let versions: Vec<_> = response
                    .versions
                    .iter()
                    .filter(|v| input.include_yanked || !v.yanked)
                    .take(input.limit)
                    .collect();

                let mut output = format!("# {} - Version History\n\n", input.name);
                output.push_str(&format!("Showing {} versions:\n\n", versions.len()));

                for v in versions {
                    let yanked = if v.yanked { " [YANKED]" } else { "" };
                    output.push_str(&format!("## v{}{}\n", v.num, yanked));
                    output.push_str(&format!("- Released: {}\n", v.created_at.date_naive()));
                    output.push_str(&format!("- Downloads: {}\n", format_number(v.downloads)));
                    if let Some(license) = &v.license {
                        output.push_str(&format!("- License: {}\n", license));
                    }
                    if let Some(msrv) = &v.rust_version {
                        output.push_str(&format!("- MSRV: {}\n", msrv));
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
    async fn versions_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/no-such-crate"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"name": "no-such-crate"}))
            .await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn versions_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/my-crate"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "my-crate"})).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn versions_empty_after_filter() {
        // All versions are yanked; with include_yanked=false (default) the list is empty.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/my-crate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "my-crate",
                    "max_version": "0.1.0",
                    "downloads": 0,
                    "created_at": "2025-01-01T00:00:00.000000Z",
                    "updated_at": "2025-01-01T00:00:00.000000Z"
                },
                "versions": [
                    {
                        "num": "0.1.0",
                        "yanked": true,
                        "created_at": "2025-01-01T00:00:00.000000Z",
                        "downloads": 0
                    }
                ]
            })))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "my-crate"})).await;
        assert!(!result.is_error);
        assert!(result.all_text().contains("Showing 0 versions"));
    }
}
