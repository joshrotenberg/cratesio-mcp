//! Resource template for crate information
//!
//! Exposes crate info as resources via URI template: crates://{name}/info

use std::collections::HashMap;
use std::sync::Arc;

use tower_mcp::protocol::{ReadResourceResult, ResourceContent};
use tower_mcp::resource::{ResourceTemplate, ResourceTemplateBuilder};

use crate::state::{AppState, format_number};

pub fn build(state: Arc<AppState>) -> ResourceTemplate {
    ResourceTemplateBuilder::new("crates://{name}/info")
        .name("Crate Information")
        .description("Get detailed information about a crate by name")
        .mime_type("text/markdown")
        .handler(move |uri: String, vars: HashMap<String, String>| {
            let state = state.clone();
            async move {
                let name = vars.get("name").cloned().unwrap_or_default();

                let response =
                    state.client.get_crate(&name).await.map_err(|e| {
                        tower_mcp::Error::tool(format!("Crates.io API error: {}", e))
                    })?;

                let c = &response.crate_data;

                let mut content = format!("# {}\n\n", c.name);

                if let Some(desc) = &c.description {
                    content.push_str(&format!("{}\n\n", desc.trim()));
                }

                content.push_str("## Stats\n\n");
                content.push_str(&format!("- **Version:** {}\n", c.max_version));
                content.push_str(&format!(
                    "- **Downloads:** {}\n",
                    format_number(c.downloads)
                ));
                content.push_str(&format!("- **Created:** {}\n", c.created_at.date_naive()));
                content.push_str(&format!("- **Updated:** {}\n", c.updated_at.date_naive()));

                if let Some(repo) = &c.repository {
                    content.push_str(&format!("\n**Repository:** {}\n", repo));
                }

                if let Some(docs) = &c.documentation {
                    content.push_str(&format!("**Documentation:** {}\n", docs));
                }

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
            "documentation": "https://docs.rs/tower-mcp",
            "repository": "https://github.com/joshrotenberg/tower-mcp"
        },
        "versions": []
    }"#;

    #[tokio::test]
    async fn crate_info_resource_returns_content() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/tower-mcp"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(GET_CRATE_JSON, "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let template = build(state);

        let vars = HashMap::from([("name".to_string(), "tower-mcp".to_string())]);
        let result = template
            .read("crates://tower-mcp/info", vars)
            .await
            .unwrap();

        assert_eq!(result.contents.len(), 1);
        assert_eq!(result.contents[0].uri, "crates://tower-mcp/info");
        assert_eq!(
            result.contents[0].mime_type.as_deref(),
            Some("text/markdown")
        );
        let text = result.contents[0].text.as_deref().unwrap();
        assert!(text.contains("# tower-mcp"));
        assert!(text.contains("Tower-native MCP implementation"));
        assert!(text.contains("**Version:** 0.6.0"));
        assert!(text.contains("**Downloads:** 1.7K"));
        assert!(text.contains("**Repository:**"));
        assert!(text.contains("**Documentation:**"));
    }

    #[tokio::test]
    async fn crate_info_resource_not_found() {
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
        let result = template.read("crates://nonexistent/info", vars).await;

        assert!(result.is_err());
    }

    #[test]
    fn crate_info_template_definition() {
        let state = test_state("http://unused");
        let template = build(state);
        let def = template.definition();

        assert_eq!(def.uri_template, "crates://{name}/info");
        assert_eq!(def.name, "Crate Information");
        assert_eq!(
            def.description.as_deref(),
            Some("Get detailed information about a crate by name")
        );
        assert_eq!(def.mime_type.as_deref(), Some("text/markdown"));
    }

    #[test]
    fn crate_info_template_uri_matching() {
        let state = test_state("http://unused");
        let template = build(state);

        let vars = template.match_uri("crates://serde/info").unwrap();
        assert_eq!(vars.get("name"), Some(&"serde".to_string()));

        assert!(template.match_uri("crates://serde/readme").is_none());
        assert!(template.match_uri("crates://serde/docs").is_none());
    }
}
