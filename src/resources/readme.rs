//! Resource template for crate README content
//!
//! Exposes crate READMEs as resources via URI template: crates://{name}/readme

use std::collections::HashMap;
use std::sync::Arc;

use tower_mcp::protocol::{ReadResourceResult, ResourceContent};
use tower_mcp::resource::{ResourceTemplate, ResourceTemplateBuilder};

use crate::state::AppState;

/// Build the `crates://{name}/readme` resource template.
pub fn build(state: Arc<AppState>) -> ResourceTemplate {
    ResourceTemplateBuilder::new("crates://{name}/readme")
        .name("Crate README")
        .description("Get the README content for a crate")
        .mime_type("text/markdown")
        .handler(move |uri: String, vars: HashMap<String, String>| {
            let state = state.clone();
            async move {
                let name = vars.get("name").cloned().unwrap_or_default();

                let response =
                    state.client.get_crate(&name).await.map_err(|e| {
                        tower_mcp::Error::tool(format!("Crates.io API error: {}", e))
                    })?;

                let version = response.crate_data.max_version.clone();

                let readme = state
                    .client
                    .crate_readme(&name, &version)
                    .await
                    .map_err(|e| tower_mcp::Error::tool(format!("Crates.io API error: {}", e)))?;

                let content = if readme.trim().is_empty() {
                    format!("No README found for {} v{}", name, version)
                } else {
                    format!("# {} v{} - README\n\n{}", name, version, readme)
                };

                Ok(ReadResourceResult {
                    contents: vec![ResourceContent {
                        uri,
                        mime_type: Some("text/markdown".to_string()),
                        text: Some(content),
                        blob: None,
                        meta: None,
                    }],
                    meta: None,
                })
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use tokio::sync::RwLock;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::client::CratesIoClient;
    use crate::client::docsrs::DocsRsClient;
    use crate::client::osv::OsvClient;
    use crate::docs::cache::DocsCache;

    fn test_state(crates_url: &str) -> Arc<AppState> {
        Arc::new(AppState {
            client: CratesIoClient::with_base_url("test", Duration::from_millis(0), crates_url)
                .unwrap(),
            docsrs_client: DocsRsClient::new("test").unwrap(),
            osv_client: OsvClient::new("test").unwrap(),
            docs_cache: DocsCache::new(10, Duration::from_secs(3600)),
            recent_searches: RwLock::new(Vec::new()),
        })
    }

    const GET_CRATE_JSON: &str = r#"{
        "crate": {
            "name": "tower-mcp",
            "updated_at": "2026-02-11T13:21:51.089324Z",
            "keywords": [],
            "categories": [],
            "created_at": "2026-01-28T16:29:05.281129Z",
            "downloads": 1721,
            "recent_downloads": 1721,
            "max_version": "0.6.0",
            "max_stable_version": "0.6.0",
            "description": "Tower-native MCP implementation",
            "repository": "https://github.com/joshrotenberg/tower-mcp"
        },
        "versions": []
    }"#;

    #[tokio::test]
    async fn readme_resource_returns_content() {
        let server = MockServer::start().await;
        let readme_text = "# tower-mcp\n\nA Tower-native MCP implementation.";

        Mock::given(method("GET"))
            .and(path("/crates/tower-mcp"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(GET_CRATE_JSON, "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/tower-mcp/0.6.0/readme"))
            .respond_with(ResponseTemplate::new(200).set_body_string(readme_text))
            .expect(1)
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let template = build(state);

        let vars = HashMap::from([("name".to_string(), "tower-mcp".to_string())]);
        let result = template
            .read("crates://tower-mcp/readme", vars)
            .await
            .unwrap();

        assert_eq!(result.contents.len(), 1);
        assert_eq!(result.contents[0].uri, "crates://tower-mcp/readme");
        assert_eq!(
            result.contents[0].mime_type.as_deref(),
            Some("text/markdown")
        );
        let text = result.contents[0].text.as_deref().unwrap();
        assert!(text.contains("# tower-mcp v0.6.0 - README"));
        assert!(text.contains("A Tower-native MCP implementation."));
    }

    #[tokio::test]
    async fn readme_resource_empty_readme() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/tower-mcp"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(GET_CRATE_JSON, "application/json"),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/tower-mcp/0.6.0/readme"))
            .respond_with(ResponseTemplate::new(200).set_body_string("  "))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let template = build(state);

        let vars = HashMap::from([("name".to_string(), "tower-mcp".to_string())]);
        let result = template
            .read("crates://tower-mcp/readme", vars)
            .await
            .unwrap();

        let text = result.contents[0].text.as_deref().unwrap();
        assert_eq!(text, "No README found for tower-mcp v0.6.0");
    }

    #[tokio::test]
    async fn readme_resource_crate_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/nonexistent"))
            .respond_with(
                ResponseTemplate::new(404)
                    .set_body_raw(r#"{"errors":[{"detail":"Not Found"}]}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let template = build(state);

        let vars = HashMap::from([("name".to_string(), "nonexistent".to_string())]);
        let result = template.read("crates://nonexistent/readme", vars).await;

        assert!(result.is_err());
    }

    #[test]
    fn readme_template_definition() {
        let state = Arc::new(AppState {
            client: CratesIoClient::with_base_url(
                "test",
                Duration::from_millis(0),
                "http://unused",
            )
            .unwrap(),
            docsrs_client: DocsRsClient::new("test").unwrap(),
            osv_client: OsvClient::new("test").unwrap(),
            docs_cache: DocsCache::new(1, Duration::from_secs(1)),
            recent_searches: RwLock::new(Vec::new()),
        });

        let template = build(state);
        let def = template.definition();

        assert_eq!(def.uri_template, "crates://{name}/readme");
        assert_eq!(def.name, "Crate README");
        assert_eq!(
            def.description.as_deref(),
            Some("Get the README content for a crate")
        );
        assert_eq!(def.mime_type.as_deref(), Some("text/markdown"));
    }

    #[test]
    fn readme_template_uri_matching() {
        let state = Arc::new(AppState {
            client: CratesIoClient::with_base_url(
                "test",
                Duration::from_millis(0),
                "http://unused",
            )
            .unwrap(),
            docsrs_client: DocsRsClient::new("test").unwrap(),
            osv_client: OsvClient::new("test").unwrap(),
            docs_cache: DocsCache::new(1, Duration::from_secs(1)),
            recent_searches: RwLock::new(Vec::new()),
        });

        let template = build(state);

        let vars = template.match_uri("crates://serde/readme").unwrap();
        assert_eq!(vars.get("name"), Some(&"serde".to_string()));

        assert!(template.match_uri("crates://serde/info").is_none());
        assert!(template.match_uri("crates://serde/docs").is_none());
    }
}
