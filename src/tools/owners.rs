//! Get owners tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for getting owners
#[derive(Debug, Deserialize, JsonSchema)]
pub struct OwnersInput {
    /// Crate name
    name: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_owners")
        .title("Get Owners")
        .description(
            "Get the owners/maintainers of a crate. Shows GitHub usernames \
             and team memberships.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<OwnersInput>| async move {
                let owners = state
                    .client
                    .crate_owners(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                let mut output = format!("# {} - Owners\n\n", input.name);

                let (users, teams): (Vec<_>, Vec<_>) = owners
                    .iter()
                    .partition(|o| o.kind.as_deref() == Some("user"));

                if !users.is_empty() {
                    output.push_str("## Users\n\n");
                    for owner in &users {
                        output.push_str(&format!("- **{}**", owner.login));
                        if let Some(name) = &owner.name {
                            output.push_str(&format!(" ({})", name));
                        }
                        output.push('\n');
                    }
                }

                if !teams.is_empty() {
                    output.push_str("\n## Teams\n\n");
                    for owner in &teams {
                        output.push_str(&format!("- **{}**\n", owner.login));
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

    // No meaningful empty case: every valid crate on crates.io has at least one owner.

    #[tokio::test]
    async fn owners_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/nonexistent/owners"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "nonexistent"})).await;

        assert!(result.is_error);
    }

    #[tokio::test]
    async fn owners_api_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/my-crate/owners"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "my-crate"})).await;

        assert!(result.is_error);
    }
}
