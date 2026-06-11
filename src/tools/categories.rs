//! Get categories tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for listing categories
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CategoriesInput {
    /// Page number (default: 1)
    #[serde(default = "default_page")]
    page: u64,
    /// Results per page (default: 20, max: 100)
    #[serde(default = "default_per_page")]
    per_page: u64,
}

fn default_page() -> u64 {
    1
}

fn default_per_page() -> u64 {
    20
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_categories")
        .title("Get Categories")
        .description(
            "List crates.io categories with the number of crates in each. \
             Useful for discovering crates by domain (e.g., web-programming, \
             cryptography, database-implementations).",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<CategoriesInput>| async move {
                let response = state
                    .client
                    .categories(Some(input.page), Some(input.per_page))
                    .await
                    .tool_context("Crates.io API error")?;

                let mut output = format!(
                    "# Crates.io Categories (page {}, {} total)\n\n",
                    input.page, response.meta.total
                );

                for cat in &response.categories {
                    output.push_str(&format!(
                        "- **{}** ({} crates)\n",
                        cat.category, cat.crates_cnt
                    ));
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

    // No 404 case: /categories is a listing endpoint with no single-resource lookup.

    #[tokio::test]
    async fn categories_api_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/categories"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({})).await;

        assert!(result.is_error);
    }

    #[tokio::test]
    async fn categories_empty() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/categories"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "categories": [],
                "meta": { "total": 0 }
            })))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({})).await;

        assert!(!result.is_error);
        assert!(result.all_text().contains("0 total"));
    }
}
