//! Find alternative crates tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::client::{CratesQuery, Sort};
use crate::state::{AppState, format_number};

/// Input for finding alternative crates
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindAlternativesInput {
    /// Name of the crate to find alternatives for
    name: String,
    /// Maximum number of alternatives to return (default: 5)
    #[serde(default = "default_max_results")]
    max_results: usize,
}

fn default_max_results() -> usize {
    5
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("find_alternatives")
        .title("Find Alternatives")
        .description(
            "Find and compare alternative crates for a given crate. Uses the crate's keywords \
             to search for related crates, then returns a comparison table showing downloads, \
             recent activity, and descriptions.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<FindAlternativesInput>| async move {
                // 1. Get target crate info to extract keywords
                let target = state
                    .client
                    .get_crate(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                let crate_data = &target.crate_data;

                // 2. Build search query from first few keywords
                let keywords: Vec<String> = crate_data
                    .keywords
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .take(3)
                    .cloned()
                    .collect();

                if keywords.is_empty() {
                    return Ok(CallToolResult::text(format!(
                        "No keywords found for '{}'. Cannot search for alternatives.",
                        input.name
                    )));
                }

                let search_term = keywords.join(" ");

                // 3. Search for related crates by keyword
                let query = CratesQuery::builder()
                    .search(&search_term)
                    .sort(Sort::Downloads)
                    .per_page(25)
                    .build();

                let search_results = state
                    .client
                    .crates(query)
                    .await
                    .tool_context("Crates.io API error")?;

                // 4. Filter out the original crate and collect candidates
                let candidates: Vec<_> = search_results
                    .crates
                    .iter()
                    .filter(|c| c.name.to_lowercase() != input.name.to_lowercase())
                    .take(input.max_results)
                    .collect();

                if candidates.is_empty() {
                    return Ok(CallToolResult::text(format!(
                        "No alternatives found for '{}' using keywords: {}",
                        input.name, search_term
                    )));
                }

                // 5. Format output as markdown comparison table
                let mut output = format!(
                    "# Alternatives to `{}`\n\n",
                    input.name
                );

                // Target crate summary
                output.push_str(&format!(
                    "**Target**: {} v{} — {}\n",
                    crate_data.name,
                    crate_data.max_version,
                    crate_data.description.as_deref().unwrap_or("No description"),
                ));
                output.push_str(&format!(
                    "**Keywords searched**: {}\n\n",
                    search_term
                ));

                // Comparison table header
                output.push_str(
                    "| Crate | Version | Description | Downloads | Recent | Last Updated |\n",
                );
                output.push_str("|---|---|---|---|---|---|\n");

                // Target crate row
                output.push_str(&format!(
                    "| **{}** *(target)* | {} | {} | {} | {} | {} |\n",
                    crate_data.name,
                    crate_data.max_version,
                    crate_data
                        .description
                        .as_deref()
                        .unwrap_or("-")
                        .trim()
                        .chars()
                        .take(60)
                        .collect::<String>(),
                    format_number(crate_data.downloads),
                    crate_data
                        .recent_downloads
                        .map(format_number)
                        .unwrap_or_else(|| "-".to_string()),
                    crate_data.updated_at.date_naive(),
                ));

                // 5. Get basic info for each alternative and add to table
                for candidate in &candidates {
                    match state.client.get_crate(&candidate.name).await {
                        Ok(alt) => {
                            let c = &alt.crate_data;
                            output.push_str(&format!(
                                "| {} | {} | {} | {} | {} | {} |\n",
                                c.name,
                                c.max_version,
                                c.description
                                    .as_deref()
                                    .unwrap_or("-")
                                    .trim()
                                    .chars()
                                    .take(60)
                                    .collect::<String>(),
                                format_number(c.downloads),
                                c.recent_downloads
                                    .map(format_number)
                                    .unwrap_or_else(|| "-".to_string()),
                                c.updated_at.date_naive(),
                            ));
                        }
                        Err(_) => {
                            // Fall back to search result data if detailed fetch fails
                            output.push_str(&format!(
                                "| {} | {} | {} | {} | {} | - |\n",
                                candidate.name,
                                candidate.max_version,
                                candidate
                                    .description
                                    .as_deref()
                                    .unwrap_or("-")
                                    .trim()
                                    .chars()
                                    .take(60)
                                    .collect::<String>(),
                                format_number(candidate.downloads),
                                candidate
                                    .recent_downloads
                                    .map(format_number)
                                    .unwrap_or_else(|| "-".to_string()),
                            ));
                        }
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
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::client::CratesIoClient;
    use crate::client::docsrs::DocsRsClient;
    use crate::client::osv::OsvClient;
    use crate::docs::cache::DocsCache;
    use crate::state::AppState;

    fn test_state(crates_url: &str) -> Arc<AppState> {
        let osv_url = "http://localhost:1";
        Arc::new(AppState {
            client: CratesIoClient::with_base_url("test", Duration::from_millis(0), crates_url)
                .unwrap(),
            docsrs_client: DocsRsClient::with_base_url("test", crates_url).unwrap(),
            osv_client: OsvClient::with_base_url("test", osv_url).unwrap(),
            docs_cache: DocsCache::new(10, Duration::from_secs(3600)),
            recent_searches: RwLock::new(Vec::new()),
        })
    }

    #[tokio::test]
    async fn find_alternatives_basic() {
        let server = MockServer::start().await;

        // Target crate info
        Mock::given(method("GET"))
            .and(path("/crates/serde"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "serde",
                    "max_version": "1.0.210",
                    "description": "A generic serialization/deserialization framework",
                    "downloads": 500000000,
                    "recent_downloads": 50000000,
                    "created_at": "2015-01-01T00:00:00.000000Z",
                    "updated_at": "2026-01-01T00:00:00.000000Z",
                    "keywords": ["serialization", "serde", "encoding"],
                    "categories": ["encoding"]
                },
                "versions": [
                    {"num": "1.0.210", "yanked": false, "created_at": "2026-01-01T00:00:00.000000Z", "downloads": 1000000}
                ]
            })))
            .mount(&server)
            .await;

        // Search results for the keywords
        Mock::given(method("GET"))
            .and(path("/crates"))
            .and(query_param("q", "serialization serde encoding"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crates": [
                    {
                        "name": "serde",
                        "max_version": "1.0.210",
                        "description": "A generic serialization/deserialization framework",
                        "downloads": 500000000,
                        "recent_downloads": 50000000,
                        "created_at": "2015-01-01T00:00:00.000000Z",
                        "updated_at": "2026-01-01T00:00:00.000000Z"
                    },
                    {
                        "name": "bincode",
                        "max_version": "1.3.3",
                        "description": "A binary serialization / deserialization strategy",
                        "downloads": 80000000,
                        "recent_downloads": 8000000,
                        "created_at": "2017-01-01T00:00:00.000000Z",
                        "updated_at": "2025-06-01T00:00:00.000000Z"
                    },
                    {
                        "name": "postcard",
                        "max_version": "1.0.8",
                        "description": "A compact serializer for embedded targets",
                        "downloads": 10000000,
                        "recent_downloads": 1000000,
                        "created_at": "2020-01-01T00:00:00.000000Z",
                        "updated_at": "2025-11-01T00:00:00.000000Z"
                    }
                ],
                "meta": {"total": 3}
            })))
            .mount(&server)
            .await;

        // Alternative crate: bincode
        Mock::given(method("GET"))
            .and(path("/crates/bincode"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "bincode",
                    "max_version": "1.3.3",
                    "description": "A binary serialization / deserialization strategy",
                    "downloads": 80000000,
                    "recent_downloads": 8000000,
                    "created_at": "2017-01-01T00:00:00.000000Z",
                    "updated_at": "2025-06-01T00:00:00.000000Z"
                },
                "versions": [
                    {"num": "1.3.3", "yanked": false, "created_at": "2025-06-01T00:00:00.000000Z", "downloads": 5000000}
                ]
            })))
            .mount(&server)
            .await;

        // Alternative crate: postcard
        Mock::given(method("GET"))
            .and(path("/crates/postcard"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "postcard",
                    "max_version": "1.0.8",
                    "description": "A compact serializer for embedded targets",
                    "downloads": 10000000,
                    "recent_downloads": 1000000,
                    "created_at": "2020-01-01T00:00:00.000000Z",
                    "updated_at": "2025-11-01T00:00:00.000000Z"
                },
                "versions": [
                    {"num": "1.0.8", "yanked": false, "created_at": "2025-11-01T00:00:00.000000Z", "downloads": 500000}
                ]
            })))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "serde"})).await;

        let text = result.all_text();
        assert!(text.contains("Alternatives to `serde`"));
        assert!(text.contains("serialization serde encoding"));
        assert!(text.contains("bincode"));
        assert!(text.contains("postcard"));
        // Target should appear in table as well
        assert!(text.contains("*(target)*"));
        // serde should not appear as an alternative
        assert!(!text.contains("serde | 1.0.210"));
    }

    #[tokio::test]
    async fn find_alternatives_no_keywords() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/no-keywords-crate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "no-keywords-crate",
                    "max_version": "0.1.0",
                    "description": "A crate with no keywords",
                    "downloads": 100,
                    "created_at": "2025-01-01T00:00:00.000000Z",
                    "updated_at": "2025-01-01T00:00:00.000000Z",
                    "keywords": []
                },
                "versions": [
                    {"num": "0.1.0", "yanked": false, "created_at": "2025-01-01T00:00:00.000000Z", "downloads": 100}
                ]
            })))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"name": "no-keywords-crate"}))
            .await;

        let text = result.all_text();
        assert!(text.contains("No keywords found"));
    }

    #[tokio::test]
    async fn find_alternatives_custom_max_results() {
        let server = MockServer::start().await;

        // Target crate
        Mock::given(method("GET"))
            .and(path("/crates/tokio"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "tokio",
                    "max_version": "1.40.0",
                    "description": "An event-driven, non-blocking I/O platform",
                    "downloads": 300000000,
                    "recent_downloads": 30000000,
                    "created_at": "2016-01-01T00:00:00.000000Z",
                    "updated_at": "2026-01-01T00:00:00.000000Z",
                    "keywords": ["async", "futures", "io"]
                },
                "versions": [
                    {"num": "1.40.0", "yanked": false, "created_at": "2026-01-01T00:00:00.000000Z", "downloads": 5000000}
                ]
            })))
            .mount(&server)
            .await;

        // Search results
        Mock::given(method("GET"))
            .and(path("/crates"))
            .and(query_param("q", "async futures io"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crates": [
                    {
                        "name": "tokio",
                        "max_version": "1.40.0",
                        "description": "An event-driven, non-blocking I/O platform",
                        "downloads": 300000000,
                        "recent_downloads": 30000000,
                        "created_at": "2016-01-01T00:00:00.000000Z",
                        "updated_at": "2026-01-01T00:00:00.000000Z"
                    },
                    {
                        "name": "async-std",
                        "max_version": "1.12.0",
                        "description": "Async version of the Rust standard library",
                        "downloads": 50000000,
                        "recent_downloads": 2000000,
                        "created_at": "2019-01-01T00:00:00.000000Z",
                        "updated_at": "2024-01-01T00:00:00.000000Z"
                    }
                ],
                "meta": {"total": 2}
            })))
            .mount(&server)
            .await;

        // async-std detail
        Mock::given(method("GET"))
            .and(path("/crates/async-std"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "async-std",
                    "max_version": "1.12.0",
                    "description": "Async version of the Rust standard library",
                    "downloads": 50000000,
                    "recent_downloads": 2000000,
                    "created_at": "2019-01-01T00:00:00.000000Z",
                    "updated_at": "2024-01-01T00:00:00.000000Z"
                },
                "versions": [
                    {"num": "1.12.0", "yanked": false, "created_at": "2024-01-01T00:00:00.000000Z", "downloads": 1000000}
                ]
            })))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"name": "tokio", "max_results": 1}))
            .await;

        let text = result.all_text();
        assert!(text.contains("Alternatives to `tokio`"));
        assert!(text.contains("async-std"));
    }
}
