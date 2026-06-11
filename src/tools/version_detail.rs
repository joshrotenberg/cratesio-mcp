//! Get specific crate version detail tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, format_number};

/// Input for getting a specific version
#[derive(Debug, Deserialize, JsonSchema)]
pub struct VersionDetailInput {
    /// Crate name
    name: String,
    /// Version string (e.g. "1.0.0")
    version: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_crate_version")
        .title("Get Crate Version")
        .description(
            "Get detailed metadata for a specific crate version including \
             license, MSRV, download count, and yanked status.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<VersionDetailInput>| async move {
                let v = state
                    .client
                    .crate_version(&input.name, &input.version)
                    .await
                    .tool_context("Crates.io API error")?;

                let yanked = if v.yanked { " [YANKED]" } else { "" };
                let mut output = format!("# {} v{}{}\n\n", input.name, v.num, yanked);

                output.push_str(&format!("- **Released:** {}\n", v.created_at.date_naive()));
                output.push_str(&format!(
                    "- **Downloads:** {}\n",
                    format_number(v.downloads)
                ));

                if let Some(license) = &v.license {
                    output.push_str(&format!("- **License:** {}\n", license));
                }
                if let Some(msrv) = &v.rust_version {
                    output.push_str(&format!("- **MSRV:** {}\n", msrv));
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
    async fn version_detail_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/my-crate/9.9.9"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"name": "my-crate", "version": "9.9.9"}))
            .await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn version_detail_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/my-crate/1.0.0"))
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
}
