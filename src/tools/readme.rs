//! Get crate readme tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for getting a crate's README
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadmeInput {
    /// Crate name
    name: String,
    /// Version (defaults to latest)
    #[serde(default)]
    version: Option<String>,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_crate_readme")
        .title("Get Crate Readme")
        .description(
            "Get the README content for a crate version. Returns the rendered README \
             from the crate's published package. Defaults to the latest version.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<ReadmeInput>| async move {
                // If no version specified, get the latest
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

                let readme = state
                    .client
                    .crate_readme(&input.name, &version)
                    .await
                    .tool_context("Crates.io API error")?;

                if readme.trim().is_empty() {
                    Ok(CallToolResult::text(format!(
                        "No README found for {} v{}",
                        input.name, version
                    )))
                } else {
                    Ok(CallToolResult::text(format!(
                        "# {} v{} - README\n\n{}",
                        input.name, version, readme
                    )))
                }
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
    async fn readme_crate_not_found() {
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
    async fn readme_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/my-crate/1.0.0/readme"))
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
    async fn readme_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/my-crate/1.0.0/readme"))
            .respond_with(ResponseTemplate::new(200).set_body_string("   "))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"name": "my-crate", "version": "1.0.0"}))
            .await;
        assert!(result.all_text().contains("No README found"));
    }
}
