//! Get keyword detail tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for getting a keyword
#[derive(Debug, Deserialize, JsonSchema)]
pub struct KeywordDetailInput {
    /// Keyword ID (e.g. "async", "cli", "parser", "serialization")
    id: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_keyword")
        .title("Get Keyword")
        .description(
            "Get details about a specific crates.io keyword, \
             including the number of crates using it.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<KeywordDetailInput>| async move {
                let kw = state
                    .client
                    .keyword(&input.id)
                    .await
                    .tool_context("Crates.io API error")?;

                let output = format!(
                    "# Keyword: {}\n\n\
                     **Crates:** {}\n\
                     **Browse:** https://crates.io/keywords/{}\n",
                    kw.keyword, kw.crates_cnt, kw.keyword
                );

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
    async fn keyword_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/keywords/nonexistent"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"id": "nonexistent"})).await;

        assert!(result.is_error);
    }

    #[tokio::test]
    async fn keyword_api_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/keywords/async"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"id": "async"})).await;

        assert!(result.is_error);
    }
}
