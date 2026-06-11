//! Get user download stats tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, format_number};

/// Input for getting user download statistics
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UserStatsInput {
    /// GitHub username
    username: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_user_stats")
        .title("Get User Stats")
        .description(
            "Get download statistics for a crates.io user. \
             Shows total downloads across all of the user's crates.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<UserStatsInput>| async move {
                let user = state
                    .client
                    .user(&input.username)
                    .await
                    .tool_context("Crates.io API error")?;

                let stats = state
                    .client
                    .user_stats(user.id)
                    .await
                    .tool_context("Crates.io API error")?;

                let mut output = format!("# User Stats: {}\n\n", user.login);

                if let Some(name) = &user.name {
                    output.push_str(&format!("**Name:** {}\n\n", name));
                }

                output.push_str(&format!(
                    "**Total downloads:** {}\n",
                    format_number(stats.total_downloads)
                ));

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
    async fn user_stats_user_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/ghost"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"username": "ghost"})).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn user_stats_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/someuser"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"username": "someuser"})).await;
        assert!(result.is_error);
    }
}
