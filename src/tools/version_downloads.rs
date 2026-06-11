//! Get per-version download statistics tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, format_number};

/// Input for getting per-version downloads
#[derive(Debug, Deserialize, JsonSchema)]
pub struct VersionDownloadsInput {
    /// Crate name
    name: String,
    /// Version (defaults to latest)
    #[serde(default)]
    version: Option<String>,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_version_downloads")
        .title("Get Version Downloads")
        .description(
            "Get daily download statistics for a specific crate version. \
             Shows the download trend over the last 90 days for that version.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>,
             Json(input): Json<VersionDownloadsInput>| async move {
                let version = match input.version {
                    Some(v) => v,
                    None => {
                        let crate_info = state
                            .client
                            .get_crate(&input.name)
                            .await
                            .tool_context("Crates.io API error")?;
                        crate_info.crate_data.max_version.clone()
                    }
                };

                let response = state
                    .client
                    .version_downloads(&input.name, &version)
                    .await
                    .tool_context("Crates.io API error")?;

                let total: u64 = response.version_downloads.iter().map(|v| v.downloads).sum();

                let mut output = format!(
                    "# {} v{} - Download Statistics\n\n\
                     **Total (last 90 days):** {}\n\n",
                    input.name,
                    version,
                    format_number(total)
                );

                // Show daily data, most recent first
                let mut entries: Vec<_> = response
                    .version_downloads
                    .iter()
                    .filter(|vd| vd.downloads > 0)
                    .collect();
                entries.sort_by(|a, b| b.date.cmp(&a.date));

                if !entries.is_empty() {
                    output.push_str("## Daily Downloads\n\n");
                    output.push_str("| Date | Downloads |\n");
                    output.push_str("|------|----------|\n");
                    for vd in entries.iter().take(30) {
                        let date = vd.date.as_deref().unwrap_or("unknown");
                        output.push_str(&format!(
                            "| {} | {} |\n",
                            date,
                            format_number(vd.downloads)
                        ));
                    }
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
    async fn version_downloads_crate_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/no-such-crate"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        // no version specified, so tool must fetch crate first
        let result = tool
            .call(serde_json::json!({"name": "no-such-crate"}))
            .await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn version_downloads_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/my-crate/1.0.0/downloads"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"name": "my-crate", "version": "1.0.0"}))
            .await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn version_downloads_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/my-crate/1.0.0/downloads"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "version_downloads": []
            })))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"name": "my-crate", "version": "1.0.0"}))
            .await;
        // no error and the header line is present; no Daily Downloads table
        assert!(!result.is_error);
        assert!(result.all_text().contains("Download Statistics"));
    }
}
