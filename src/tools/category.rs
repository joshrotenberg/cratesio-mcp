//! Get category detail tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for getting a category
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CategoryInput {
    /// Category slug (e.g. "command-line-utilities", "web-programming", "cryptography")
    slug: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_category")
        .title("Get Category")
        .description(
            "Get details about a specific crates.io category by slug, \
             including its description and crate count.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<CategoryInput>| async move {
                let cat = state
                    .client
                    .category(&input.slug)
                    .await
                    .tool_context("Crates.io API error")?;

                let mut output = format!("# Category: {}\n\n", cat.category);

                if let Some(desc) = &cat.description {
                    output.push_str(&format!("{}\n\n", desc));
                }

                output.push_str(&format!("**Crates:** {}\n", cat.crates_cnt));

                if let Some(slug) = &cat.slug {
                    output.push_str(&format!(
                        "**Browse:** https://crates.io/categories/{}\n",
                        slug
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

    #[tokio::test]
    async fn category_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/categories/nonexistent-slug"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"slug": "nonexistent-slug"}))
            .await;

        assert!(result.is_error);
    }

    #[tokio::test]
    async fn category_api_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/categories/web-programming"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"slug": "web-programming"}))
            .await;

        assert!(result.is_error);
    }
}
