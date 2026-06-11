//! Get crate authors tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for getting crate authors
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuthorsInput {
    /// Crate name
    name: String,
    /// Version (defaults to latest)
    #[serde(default)]
    version: Option<String>,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_crate_authors")
        .title("Get Crate Authors")
        .description(
            "Get the authors of a specific crate version. Authors are the people \
             listed in the Cargo.toml [package].authors field.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<AuthorsInput>| async move {
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

                let authors = state
                    .client
                    .crate_authors(&input.name, &version)
                    .await
                    .tool_context("Crates.io API error")?;

                let mut output = format!("# {} v{} - Authors\n\n", input.name, version);

                if authors.names.is_empty() {
                    output.push_str("No authors listed for this version.\n");
                } else {
                    for name in &authors.names {
                        output.push_str(&format!("- {}\n", name));
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
    async fn authors_crate_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/nonexistent"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "nonexistent"})).await;

        assert!(result.is_error);
    }

    #[tokio::test]
    async fn authors_api_error() {
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
    async fn authors_empty() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/my-crate/1.0.0/authors"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "meta": { "names": [] },
                "users": []
            })))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"name": "my-crate", "version": "1.0.0"}))
            .await;

        assert!(!result.is_error);
        assert!(result.all_text().contains("No authors listed"));
    }
}
