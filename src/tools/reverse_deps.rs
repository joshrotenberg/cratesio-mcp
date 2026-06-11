//! Get reverse dependencies tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::protocol::{LogLevel, LoggingMessageParams};
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Context, Json, State},
};

use crate::state::AppState;

/// Input for getting reverse dependencies
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReverseDepsInput {
    /// Crate name
    name: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_reverse_dependencies")
        .title("Get Reverse Dependencies")
        .description(
            "Get crates that depend on the specified crate (reverse dependencies). \
             Useful for understanding a crate's ecosystem impact.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>,
             ctx: Context,
             Json(input): Json<ReverseDepsInput>| async move {
                // Log the request
                ctx.send_log(LoggingMessageParams {
                    level: LogLevel::Info,
                    logger: Some("cratesio-mcp".to_string()),
                    data: serde_json::json!({
                        "action": "fetch_reverse_deps",
                        "crate": input.name
                    }),
                    meta: None,
                });

                // Send initial progress
                ctx.report_progress(0.1, Some(1.0), Some("Fetching reverse dependencies..."))
                    .await;

                let response = state
                    .client
                    .crate_reverse_dependencies(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                // Update progress
                ctx.report_progress(0.8, Some(1.0), Some("Processing results..."))
                    .await;

                // Log the result count
                ctx.send_log(LoggingMessageParams {
                    level: LogLevel::Info,
                    logger: Some("cratesio-mcp".to_string()),
                    data: serde_json::json!({
                        "action": "fetch_reverse_deps_complete",
                        "crate": input.name,
                        "count": response.meta.total
                    }),
                    meta: None,
                });

                let mut output = format!(
                    "# {} - Reverse Dependencies\n\n\
                     {} crates depend on this crate (showing first {}):\n\n",
                    input.name,
                    response.meta.total,
                    response.dependencies.len()
                );

                for dep in &response.dependencies {
                    output.push_str(&format!(
                        "- **{}** v{} ({})\n",
                        dep.crate_version.crate_name, dep.crate_version.num, dep.dependency.req
                    ));
                }

                // Complete progress
                ctx.report_progress(1.0, Some(1.0), Some("Done")).await;

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
    async fn reverse_deps_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/no-such-crate/reverse_dependencies"))
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
    async fn reverse_deps_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/my-crate/reverse_dependencies"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "my-crate"})).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn reverse_deps_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/my-crate/reverse_dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": [],
                "versions": [],
                "meta": {"total": 0}
            })))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "my-crate"})).await;
        assert!(result.all_text().contains("0 crates depend"));
    }
}
