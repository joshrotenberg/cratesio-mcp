//! Resource template for crate documentation
//!
//! Exposes crate docs as resources via URI template: crates://{name}/docs

use std::collections::HashMap;
use std::sync::Arc;

use tower_mcp::protocol::{ReadResourceResult, ResourceContent};
use tower_mcp::resource::{ResourceTemplate, ResourceTemplateBuilder};

use crate::docs::format;
use crate::state::AppState;

/// Build the `crates://{name}/docs` resource template.
pub fn build(state: Arc<AppState>) -> ResourceTemplate {
    ResourceTemplateBuilder::new("crates://{name}/docs")
        .name("Crate Documentation")
        .description("Get the documentation structure for a crate from docs.rs")
        .mime_type("text/markdown")
        .handler(move |uri: String, vars: HashMap<String, String>| {
            let state = state.clone();
            async move {
                let name = vars.get("name").cloned().unwrap_or_default();

                let krate = state
                    .docs_cache
                    .get_or_fetch(&state.docsrs_client, &name, "latest")
                    .await
                    .map_err(|e| tower_mcp::Error::tool(format!("docs.rs fetch error: {}", e)))?;

                let output = format::format_module_listing(&krate, &krate.root);

                Ok(ReadResourceResult {
                    contents: vec![ResourceContent {
                        uri,
                        mime_type: Some("text/markdown".to_string()),
                        text: Some(output),
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
    use crate::state::AppState;

    fn test_state(docsrs_url: &str) -> Arc<AppState> {
        Arc::new(AppState {
            client: CratesIoClient::with_base_url(
                "test",
                Duration::from_millis(0),
                "http://unused",
            )
            .unwrap(),
            docsrs_client: DocsRsClient::with_base_url("test", docsrs_url).unwrap(),
            osv_client: OsvClient::new("test").unwrap(),
            docs_cache: DocsCache::new(10, Duration::from_secs(3600)),
            recent_searches: RwLock::new(Vec::new()),
        })
    }

    /// Minimal valid rustdoc JSON that parses into a `Crate`.
    fn synthetic_crate_json() -> Vec<u8> {
        let json = serde_json::json!({
            "root": 0,
            "crate_version": "1.0.0",
            "includes_private": false,
            "index": {},
            "paths": {},
            "external_crates": {},
            "target": {
                "triple": "x86_64-unknown-linux-gnu",
                "target_features": []
            },
            "format_version": rustdoc_types::FORMAT_VERSION
        });
        serde_json::to_vec(&json).unwrap()
    }

    #[tokio::test]
    async fn docs_resource_returns_content() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crate/serde/latest/json.gz"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(synthetic_crate_json())
                    .insert_header("content-type", "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let template = build(state);

        let vars = HashMap::from([("name".to_string(), "serde".to_string())]);
        let result = template.read("crates://serde/docs", vars).await.unwrap();

        assert_eq!(result.contents.len(), 1);
        assert_eq!(result.contents[0].uri, "crates://serde/docs");
        assert_eq!(
            result.contents[0].mime_type.as_deref(),
            Some("text/markdown")
        );
        assert!(result.contents[0].text.is_some());
    }

    #[tokio::test]
    async fn docs_resource_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crate/nonexistent/latest/json.gz"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let template = build(state);

        let vars = HashMap::from([("name".to_string(), "nonexistent".to_string())]);
        let result = template.read("crates://nonexistent/docs", vars).await;

        assert!(result.is_err());
    }

    #[test]
    fn docs_template_definition() {
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

        assert_eq!(def.uri_template, "crates://{name}/docs");
        assert_eq!(def.name, "Crate Documentation");
        assert_eq!(
            def.description.as_deref(),
            Some("Get the documentation structure for a crate from docs.rs")
        );
        assert_eq!(def.mime_type.as_deref(), Some("text/markdown"));
    }

    #[test]
    fn docs_template_uri_matching() {
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

        let vars = template.match_uri("crates://tokio/docs").unwrap();
        assert_eq!(vars.get("name"), Some(&"tokio".to_string()));

        assert!(template.match_uri("crates://tokio/info").is_none());
        assert!(template.match_uri("crates://tokio/readme").is_none());
    }
}
