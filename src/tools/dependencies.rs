//! Get dependencies tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for getting dependencies
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DependenciesInput {
    /// Crate name
    name: String,
    /// Version (default: latest)
    #[serde(default)]
    version: Option<String>,
    /// Include dev dependencies
    #[serde(default)]
    include_dev: bool,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_dependencies")
        .title("Get Dependencies")
        .description(
            "Get dependencies for a crate version. Shows required and optional deps, \
             version requirements, and whether they're build or dev dependencies.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<DependenciesInput>| async move {
                // Get crate info first to find version
                let crate_response = state
                    .client
                    .get_crate(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                let version = input
                    .version
                    .as_deref()
                    .unwrap_or(&crate_response.crate_data.max_version);

                let deps = state
                    .client
                    .crate_dependencies(&input.name, version)
                    .await
                    .tool_context("Crates.io API error")?;

                let (normal, dev, build): (Vec<_>, Vec<_>, Vec<_>) =
                    deps.iter().fold((vec![], vec![], vec![]), |mut acc, d| {
                        match d.kind.as_str() {
                            "dev" => acc.1.push(d),
                            "build" => acc.2.push(d),
                            _ => acc.0.push(d),
                        }
                        acc
                    });

                let mut output = format!("# {} v{} - Dependencies\n\n", input.name, version);

                if !normal.is_empty() {
                    output.push_str("## Dependencies\n\n");
                    for d in &normal {
                        let optional = if d.optional { " (optional)" } else { "" };
                        output.push_str(&format!("- **{}** {}{}\n", d.crate_id, d.req, optional));
                    }
                    output.push('\n');
                }

                if !build.is_empty() {
                    output.push_str("## Build Dependencies\n\n");
                    for d in &build {
                        let optional = if d.optional { " (optional)" } else { "" };
                        output.push_str(&format!("- **{}** {}{}\n", d.crate_id, d.req, optional));
                    }
                    output.push('\n');
                }

                if input.include_dev && !dev.is_empty() {
                    output.push_str("## Dev Dependencies\n\n");
                    for d in &dev {
                        let optional = if d.optional { " (optional)" } else { "" };
                        output.push_str(&format!("- **{}** {}{}\n", d.crate_id, d.req, optional));
                    }
                    output.push('\n');
                }

                let total =
                    normal.len() + build.len() + if input.include_dev { dev.len() } else { 0 };
                output.push_str(&format!("**Total: {} dependencies**\n", total));

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

    #[test]
    fn input_deserializes_without_version_key() {
        let input: super::DependenciesInput =
            serde_json::from_value(serde_json::json!({"name": "serde"})).unwrap();
        assert!(input.version.is_none());
    }

    #[tokio::test]
    async fn dependencies_crate_not_found() {
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
    async fn dependencies_api_error() {
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
    async fn dependencies_empty() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/my-crate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "my-crate",
                    "max_version": "1.0.0",
                    "downloads": 0,
                    "created_at": "2025-01-01T00:00:00.000000Z",
                    "updated_at": "2025-01-01T00:00:00.000000Z"
                },
                "versions": [
                    {"num": "1.0.0", "yanked": false, "created_at": "2025-01-01T00:00:00.000000Z", "downloads": 0}
                ]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/my-crate/1.0.0/dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": []
            })))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "my-crate"})).await;

        assert!(!result.is_error);
        assert!(result.all_text().contains("Total: 0 dependencies"));
    }
}
