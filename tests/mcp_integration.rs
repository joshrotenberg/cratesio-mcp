//! MCP integration tests using tower-mcp's TestClient + wiremock.
//!
//! These tests exercise the full JSON-RPC pipeline: client request -> router ->
//! tool/resource/prompt handler -> wiremock mock -> formatted response.

use std::collections::HashMap;
use std::sync::Arc;

use cratesio_mcp::{prompts, resources, state::AppState, tools};
use serde_json::json;
use tower_mcp::{McpRouter, TestClient};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── Helpers ────────────────────────────────────────────────────────────────

fn test_state(server: &MockServer) -> Arc<AppState> {
    Arc::new(AppState::with_base_url(&server.uri()).expect("failed to create test state"))
}

fn test_router(state: Arc<AppState>) -> McpRouter {
    McpRouter::new()
        .server_info("cratesio-mcp", "0.1.0")
        .tool(tools::search::build(state.clone()))
        .tool(tools::info::build(state.clone()))
        .tool(tools::versions::build(state.clone()))
        .tool(tools::dependencies::build(state.clone()))
        .tool(tools::reverse_deps::build(state.clone()))
        .tool(tools::downloads::build(state.clone()))
        .tool(tools::owners::build(state.clone()))
        .tool(tools::summary::build(state.clone()))
        .tool(tools::authors::build(state.clone()))
        .tool(tools::user::build(state.clone()))
        .tool(tools::readme::build(state.clone()))
        .tool(tools::categories::build(state.clone()))
        .tool(tools::keywords::build(state.clone()))
        .tool(tools::version_downloads::build(state.clone()))
        .tool(tools::version_detail::build(state.clone()))
        .tool(tools::category::build(state.clone()))
        .tool(tools::keyword_detail::build(state.clone()))
        .tool(tools::features::build(state.clone()))
        .resource(resources::recent_searches::build(state.clone()))
        .resource_template(resources::crate_info::build(state.clone()))
        .resource_template(resources::readme::build(state.clone()))
        .resource_template(resources::docs::build(state.clone()))
        .prompt(prompts::analyze::build())
        .prompt(prompts::compare::build())
}

async fn initialized_client(server: &MockServer) -> TestClient {
    let state = test_state(server);
    let router = test_router(state);
    let mut client = TestClient::from_router(router);
    client.initialize().await;
    client
}

// ── Mock JSON constants ────────────────────────────────────────────────────

const GET_CRATE_JSON: &str = r#"{
    "crate": {
        "name": "tower-mcp",
        "updated_at": "2026-02-11T13:21:51.089324Z",
        "keywords": ["ai", "mcp"],
        "categories": ["asynchronous"],
        "created_at": "2026-01-28T16:29:05.281129Z",
        "downloads": 1721,
        "recent_downloads": 1721,
        "max_version": "0.6.0",
        "max_stable_version": "0.6.0",
        "description": "Tower-native MCP implementation",
        "homepage": null,
        "documentation": "https://docs.rs/tower-mcp",
        "repository": "https://github.com/joshrotenberg/tower-mcp"
    },
    "versions": [
        {
            "num": "0.6.0",
            "yanked": false,
            "created_at": "2026-02-11T13:21:51.089324Z",
            "downloads": 119,
            "license": "MIT OR Apache-2.0",
            "rust_version": "1.90"
        },
        {
            "num": "0.5.0",
            "yanked": false,
            "created_at": "2026-02-06T01:00:00.000000Z",
            "downloads": 502,
            "license": "MIT OR Apache-2.0"
        }
    ]
}"#;

const SEARCH_JSON: &str = r#"{
    "crates": [
        {
            "name": "tower-mcp",
            "max_version": "0.6.0",
            "description": "Tower-native MCP implementation",
            "downloads": 1721,
            "recent_downloads": 1721,
            "created_at": "2026-01-28T16:29:05.281129Z",
            "updated_at": "2026-02-11T13:21:51.089324Z",
            "repository": "https://github.com/joshrotenberg/tower-mcp"
        }
    ],
    "meta": { "total": 1 }
}"#;

const SUMMARY_JSON: &str = r#"{
    "num_crates": 180000,
    "num_downloads": 50000000000,
    "new_crates": [
        {
            "name": "new-crate",
            "max_version": "0.1.0",
            "description": "A brand new crate",
            "downloads": 5,
            "created_at": "2026-02-22T00:00:00.000000Z",
            "updated_at": "2026-02-22T00:00:00.000000Z"
        }
    ],
    "most_downloaded": [
        {
            "name": "serde",
            "max_version": "1.0.219",
            "description": "A serialization framework",
            "downloads": 400000000,
            "created_at": "2015-01-01T00:00:00.000000Z",
            "updated_at": "2026-01-15T00:00:00.000000Z"
        }
    ],
    "just_updated": [],
    "popular_keywords": [
        { "keyword": "serde", "crates_cnt": 5000 }
    ],
    "popular_categories": [
        { "category": "No standard library", "crates_cnt": 8000 }
    ]
}"#;

const OWNERS_JSON: &str = r#"{
    "users": [
        {
            "id": 87681,
            "login": "joshrotenberg",
            "kind": "user",
            "url": "https://github.com/joshrotenberg",
            "name": "Josh Rotenberg",
            "avatar": "https://avatars.githubusercontent.com/u/3231?v=4"
        }
    ]
}"#;

const DOWNLOADS_JSON: &str = r#"{
    "version_downloads": [
        { "version": 100, "downloads": 50, "date": "2026-02-20" },
        { "version": 100, "downloads": 42, "date": "2026-02-21" }
    ]
}"#;

const DEPENDENCIES_JSON: &str = r#"{
    "dependencies": [
        {
            "crate_id": "tokio",
            "req": "^1",
            "kind": "normal",
            "optional": false,
            "version_id": 100
        },
        {
            "crate_id": "wiremock",
            "req": "^0.6",
            "kind": "dev",
            "optional": false,
            "version_id": 100
        }
    ]
}"#;

const REVERSE_DEPS_JSON: &str = r#"{
    "dependencies": [
        {
            "crate_id": "tower-mcp",
            "req": "^0.6",
            "kind": "normal",
            "optional": false,
            "version_id": 200
        }
    ],
    "versions": [
        { "id": 200, "crate": "cratesio-mcp", "num": "0.1.0" }
    ],
    "meta": { "total": 1 }
}"#;

const AUTHORS_JSON: &str = r#"{
    "meta": {
        "names": ["Josh Rotenberg <josh@example.com>"]
    }
}"#;

const VERSION_DOWNLOADS_JSON: &str = r#"{
    "version_downloads": [
        { "version": 100, "downloads": 30, "date": "2026-02-20" },
        { "version": 100, "downloads": 25, "date": "2026-02-21" }
    ]
}"#;

const VERSION_JSON: &str = r#"{
    "version": {
        "num": "0.6.0",
        "yanked": false,
        "created_at": "2026-02-11T13:21:51.089324Z",
        "downloads": 119,
        "license": "MIT OR Apache-2.0",
        "rust_version": "1.90"
    }
}"#;

const CATEGORIES_JSON: &str = r#"{
    "categories": [
        {
            "category": "Asynchronous",
            "crates_cnt": 3000,
            "slug": "asynchronous",
            "description": "Crates for async programming"
        }
    ],
    "meta": { "total": 75 }
}"#;

const KEYWORDS_JSON: &str = r#"{
    "keywords": [
        { "keyword": "serde", "crates_cnt": 5000 },
        { "keyword": "async", "crates_cnt": 3000 }
    ],
    "meta": { "total": 10000 }
}"#;

// ── Helper to mount common mocks ──────────────────────────────────────────

/// Mount the GET /crates/tower-mcp mock (used by many tools).
async fn mount_get_crate(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(GET_CRATE_JSON, "application/json"))
        .mount(server)
        .await;
}

// ── Discovery tests ────────────────────────────────────────────────────────

#[tokio::test]
async fn list_tools_returns_all_18() {
    let server = MockServer::start().await;
    let mut client = initialized_client(&server).await;

    let tools = client.list_tools().await;

    assert_eq!(tools.len(), 18);
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(names.contains(&"search_crates"));
    assert!(names.contains(&"get_crate_info"));
    assert!(names.contains(&"get_crate_versions"));
    assert!(names.contains(&"get_dependencies"));
    assert!(names.contains(&"get_reverse_dependencies"));
    assert!(names.contains(&"get_downloads"));
    assert!(names.contains(&"get_owners"));
    assert!(names.contains(&"get_summary"));
    assert!(names.contains(&"get_crate_authors"));
    assert!(names.contains(&"get_user"));
    assert!(names.contains(&"get_crate_readme"));
    assert!(names.contains(&"get_categories"));
    assert!(names.contains(&"get_keywords"));
    assert!(names.contains(&"get_version_downloads"));
    assert!(names.contains(&"get_crate_version"));
    assert!(names.contains(&"get_category"));
    assert!(names.contains(&"get_keyword"));
    assert!(names.contains(&"get_crate_features"));
}

#[tokio::test]
async fn list_resources_returns_recent_searches() {
    let server = MockServer::start().await;
    let mut client = initialized_client(&server).await;

    let resources = client.list_resources().await;

    assert_eq!(resources.len(), 1);
    assert_eq!(
        resources[0].get("uri").and_then(|u| u.as_str()),
        Some("crates://recent-searches")
    );
}

#[tokio::test]
async fn list_resource_templates_returns_crate_info() {
    let server = MockServer::start().await;
    let mut client = initialized_client(&server).await;

    let result = client.send_request("resources/templates/list", None).await;
    let templates = result
        .get("resourceTemplates")
        .and_then(|v| v.as_array())
        .expect("expected resourceTemplates array");

    assert_eq!(templates.len(), 3);
    let uris: Vec<&str> = templates
        .iter()
        .filter_map(|t| t.get("uriTemplate").and_then(|u| u.as_str()))
        .collect();
    assert!(uris.contains(&"crates://{name}/info"));
    assert!(uris.contains(&"crates://{name}/readme"));
    assert!(uris.contains(&"crates://{name}/docs"));
}

#[tokio::test]
async fn list_prompts_returns_both() {
    let server = MockServer::start().await;
    let mut client = initialized_client(&server).await;

    let prompts = client.list_prompts().await;

    assert_eq!(prompts.len(), 2);
    let names: Vec<&str> = prompts
        .iter()
        .filter_map(|p| p.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(names.contains(&"analyze_crate"));
    assert!(names.contains(&"compare_crates"));
}

// ── Tool tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn tool_search_crates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SEARCH_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool("search_crates", json!({"query": "mcp"}))
        .await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("tower-mcp"));
    assert!(text.contains("Found 1 crates"));
}

#[tokio::test]
async fn tool_get_crate_info() {
    let server = MockServer::start().await;
    mount_get_crate(&server).await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool("get_crate_info", json!({"name": "tower-mcp"}))
        .await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("# tower-mcp"));
    assert!(text.contains("Tower-native MCP implementation"));
    assert!(text.contains("0.6.0"));
}

#[tokio::test]
async fn tool_get_crate_versions() {
    let server = MockServer::start().await;
    mount_get_crate(&server).await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool("get_crate_versions", json!({"name": "tower-mcp"}))
        .await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("Version History"));
    assert!(text.contains("v0.6.0"));
    assert!(text.contains("v0.5.0"));
}

#[tokio::test]
async fn tool_get_dependencies() {
    let server = MockServer::start().await;
    mount_get_crate(&server).await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/0.6.0/dependencies"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(DEPENDENCIES_JSON, "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool("get_dependencies", json!({"name": "tower-mcp"}))
        .await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("Dependencies"));
    assert!(text.contains("tokio"));
}

#[tokio::test]
async fn tool_get_reverse_dependencies() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/reverse_dependencies"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(REVERSE_DEPS_JSON, "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool("get_reverse_dependencies", json!({"name": "tower-mcp"}))
        .await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("Reverse Dependencies"));
    assert!(text.contains("cratesio-mcp"));
}

#[tokio::test]
async fn tool_get_downloads() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/downloads"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(DOWNLOADS_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool("get_downloads", json!({"name": "tower-mcp"}))
        .await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("Download Statistics"));
}

#[tokio::test]
async fn tool_get_owners() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/owners"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(OWNERS_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool("get_owners", json!({"name": "tower-mcp"}))
        .await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("Owners"));
    assert!(text.contains("joshrotenberg"));
}

#[tokio::test]
async fn tool_get_summary() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/summary"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SUMMARY_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client.call_tool("get_summary", json!({})).await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("Crates.io Summary"));
    assert!(text.contains("new-crate"));
    assert!(text.contains("serde"));
}

#[tokio::test]
async fn tool_get_crate_authors() {
    let server = MockServer::start().await;
    mount_get_crate(&server).await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/0.6.0/authors"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(AUTHORS_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool("get_crate_authors", json!({"name": "tower-mcp"}))
        .await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("Authors"));
    assert!(text.contains("Josh Rotenberg"));
}

#[tokio::test]
async fn tool_get_user() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/users/joshrotenberg"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user": {
                "login": "joshrotenberg",
                "name": "Josh Rotenberg",
                "url": "https://github.com/joshrotenberg",
                "avatar": "https://avatars.githubusercontent.com/u/3231?v=4",
                "kind": "user"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool("get_user", json!({"username": "joshrotenberg"}))
        .await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("joshrotenberg"));
    assert!(text.contains("Josh Rotenberg"));
}

#[tokio::test]
async fn tool_get_crate_readme() {
    let server = MockServer::start().await;
    mount_get_crate(&server).await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/0.6.0/readme"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("# tower-mcp\n\nAn MCP implementation."),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool("get_crate_readme", json!({"name": "tower-mcp"}))
        .await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("README"));
    assert!(text.contains("An MCP implementation"));
}

#[tokio::test]
async fn tool_get_categories() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/categories"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CATEGORIES_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client.call_tool("get_categories", json!({})).await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("Categories"));
    assert!(text.contains("Asynchronous"));
}

#[tokio::test]
async fn tool_get_keywords() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/keywords"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(KEYWORDS_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client.call_tool("get_keywords", json!({})).await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("Keywords"));
    assert!(text.contains("serde"));
    assert!(text.contains("async"));
}

#[tokio::test]
async fn tool_get_version_downloads() {
    let server = MockServer::start().await;
    mount_get_crate(&server).await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/0.6.0/downloads"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(VERSION_DOWNLOADS_JSON, "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool("get_version_downloads", json!({"name": "tower-mcp"}))
        .await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("Download Statistics"));
    assert!(text.contains("v0.6.0"));
}

#[tokio::test]
async fn tool_get_crate_version() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/0.6.0"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(VERSION_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool(
            "get_crate_version",
            json!({"name": "tower-mcp", "version": "0.6.0"}),
        )
        .await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("v0.6.0"));
    assert!(text.contains("MIT OR Apache-2.0"));
}

#[tokio::test]
async fn tool_get_category() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/categories/asynchronous"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "category": {
                "category": "Asynchronous",
                "crates_cnt": 3000,
                "slug": "asynchronous",
                "description": "Crates for async programming"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool("get_category", json!({"slug": "asynchronous"}))
        .await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("Asynchronous"));
    assert!(text.contains("3000"));
}

#[tokio::test]
async fn tool_get_keyword() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/keywords/serde"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "keyword": {
                "keyword": "serde",
                "crates_cnt": 5000
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool("get_keyword", json!({"id": "serde"}))
        .await;

    assert!(!result.is_error);
    let text = result.all_text();
    assert!(text.contains("serde"));
    assert!(text.contains("5000"));
}

// ── Features tool test ─────────────────────────────────────────────────────

const VERSION_WITH_FEATURES_JSON: &str = r#"{
    "version": {
        "num": "0.6.0",
        "yanked": false,
        "created_at": "2026-02-11T13:21:51.089324Z",
        "downloads": 119,
        "license": "MIT OR Apache-2.0",
        "rust_version": "1.90",
        "features": {
            "default": ["stdio"],
            "stdio": [],
            "http": ["dep:hyper", "dep:axum"]
        }
    }
}"#;

#[tokio::test]
async fn tool_get_crate_features() {
    let server = MockServer::start().await;
    mount_get_crate(&server).await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/0.6.0"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(VERSION_WITH_FEATURES_JSON, "application/json"),
        )
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool("get_crate_features", json!({"name": "tower-mcp"}))
        .await;

    let text = result.first_text().expect("expected text");
    assert!(text.contains("Feature Flags"));
    assert!(text.contains("`stdio`"));
    assert!(text.contains("`http`"));
    assert!(text.contains("`dep:hyper`"));
    assert!(text.contains("Default Features"));
    assert!(text.contains("Total: 3 feature flags"));
}

#[tokio::test]
async fn tool_get_crate_features_with_version() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/0.6.0"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(VERSION_WITH_FEATURES_JSON, "application/json"),
        )
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;
    let result = client
        .call_tool(
            "get_crate_features",
            json!({"name": "tower-mcp", "version": "0.6.0"}),
        )
        .await;

    let text = result.first_text().expect("expected text");
    assert!(text.contains("tower-mcp v0.6.0"));
    assert!(text.contains("Feature Flags"));
}

// ── Docs.rs / OSV tool tests ──────────────────────────────────────────────
//
// These tools need separate mock servers for docs.rs and/or OSV.dev,
// so they use AppState::with_all_base_urls instead of with_base_url.

fn full_test_state(
    crates_server: &MockServer,
    docsrs_server: &MockServer,
    osv_server: &MockServer,
) -> Arc<AppState> {
    Arc::new(
        AppState::with_all_base_urls(
            &crates_server.uri(),
            &docsrs_server.uri(),
            &osv_server.uri(),
        )
        .expect("failed to create test state"),
    )
}

fn full_test_router(state: Arc<AppState>) -> McpRouter {
    McpRouter::new()
        .server_info("cratesio-mcp", "0.1.0")
        .tool(tools::crate_docs::build(state.clone()))
        .tool(tools::doc_item::build(state.clone()))
        .tool(tools::search_docs::build(state.clone()))
        .tool(tools::audit::build(state.clone()))
}

async fn full_initialized_client(
    crates_server: &MockServer,
    docsrs_server: &MockServer,
    osv_server: &MockServer,
) -> TestClient {
    let state = full_test_state(crates_server, docsrs_server, osv_server);
    let router = full_test_router(state);
    let mut client = TestClient::from_router(router);
    client.initialize().await;
    client
}

/// Minimal valid rustdoc JSON for testing.
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
async fn tool_get_crate_docs() {
    let crates_server = MockServer::start().await;
    let docsrs_server = MockServer::start().await;
    let osv_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crate/serde/latest/json.gz"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(synthetic_crate_json())
                .insert_header("content-type", "application/json"),
        )
        .mount(&docsrs_server)
        .await;

    let mut client = full_initialized_client(&crates_server, &docsrs_server, &osv_server).await;
    let result = client
        .call_tool("get_crate_docs", json!({"name": "serde"}))
        .await;

    let text = result.first_text().expect("expected text");
    // Synthetic crate has empty index, so output should reflect that
    assert!(!text.is_empty());
}

#[tokio::test]
async fn tool_get_crate_docs_not_found() {
    let crates_server = MockServer::start().await;
    let docsrs_server = MockServer::start().await;
    let osv_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crate/nonexistent/latest/json.gz"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&docsrs_server)
        .await;

    let mut client = full_initialized_client(&crates_server, &docsrs_server, &osv_server).await;
    let result = client
        .call_tool("get_crate_docs", json!({"name": "nonexistent"}))
        .await;

    assert!(result.is_error);
}

#[tokio::test]
async fn tool_search_docs() {
    let crates_server = MockServer::start().await;
    let docsrs_server = MockServer::start().await;
    let osv_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crate/serde/latest/json.gz"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(synthetic_crate_json())
                .insert_header("content-type", "application/json"),
        )
        .mount(&docsrs_server)
        .await;

    let mut client = full_initialized_client(&crates_server, &docsrs_server, &osv_server).await;
    let result = client
        .call_tool(
            "search_docs",
            json!({"name": "serde", "query": "Serialize"}),
        )
        .await;

    let text = result.first_text().expect("expected text");
    // Empty index means no matches, but should not error
    assert!(text.contains("No items matching"));
}

#[tokio::test]
async fn tool_get_doc_item_not_found() {
    let crates_server = MockServer::start().await;
    let docsrs_server = MockServer::start().await;
    let osv_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crate/serde/latest/json.gz"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(synthetic_crate_json())
                .insert_header("content-type", "application/json"),
        )
        .mount(&docsrs_server)
        .await;

    let mut client = full_initialized_client(&crates_server, &docsrs_server, &osv_server).await;
    let result = client
        .call_tool(
            "get_doc_item",
            json!({"name": "serde", "item_path": "Serialize"}),
        )
        .await;

    // Item not in synthetic index, so should be an error
    assert!(result.is_error);
}

#[tokio::test]
async fn tool_audit_dependencies_clean() {
    let crates_server = MockServer::start().await;
    let docsrs_server = MockServer::start().await;
    let osv_server = MockServer::start().await;

    // Mount crate info mock
    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(GET_CRATE_JSON, "application/json"))
        .mount(&crates_server)
        .await;

    // Mount dependencies mock
    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/0.6.0/dependencies"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(DEPENDENCIES_JSON, "application/json"),
        )
        .mount(&crates_server)
        .await;

    // Mount OSV query mock (no vulns for any package)
    Mock::given(method("POST"))
        .and(path("/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"vulns": []})))
        .mount(&osv_server)
        .await;

    let mut client = full_initialized_client(&crates_server, &docsrs_server, &osv_server).await;
    let result = client
        .call_tool("audit_dependencies", json!({"name": "tower-mcp"}))
        .await;

    let text = result.first_text().expect("expected text");
    assert!(text.contains("Security Audit: tower-mcp v0.6.0"));
    assert!(text.contains("No known vulnerabilities found"));
    assert!(text.contains("**Dependencies checked**"));
    assert!(text.contains("**Vulnerabilities found**: 0"));
}

// ── Resource tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn resource_recent_searches_empty_initially() {
    let server = MockServer::start().await;
    let mut client = initialized_client(&server).await;

    let result = client.read_resource("crates://recent-searches").await;

    let text = result.first_text().expect("expected text content");
    assert_eq!(text, "[]");
}

#[tokio::test]
async fn resource_recent_searches_populated_after_search() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SEARCH_JSON, "application/json"))
        .mount(&server)
        .await;

    let mut client = initialized_client(&server).await;

    // Perform a search to populate recent searches
    client
        .call_tool("search_crates", json!({"query": "mcp"}))
        .await;

    // Now check the resource
    let result = client.read_resource("crates://recent-searches").await;
    let text = result.first_text().expect("expected text content");

    assert!(text.contains("mcp"));
    assert!(text.contains("tower-mcp"));
}

#[tokio::test]
async fn resource_template_crate_info() {
    let server = MockServer::start().await;
    mount_get_crate(&server).await;

    let mut client = initialized_client(&server).await;
    let result = client.read_resource("crates://tower-mcp/info").await;

    let text = result.first_text().expect("expected text content");
    assert!(text.contains("# tower-mcp"));
    assert!(text.contains("0.6.0"));
}

// ── Prompt tests ───────────────────────────────────────────────────────────

#[tokio::test]
async fn prompt_analyze_crate() {
    let server = MockServer::start().await;
    let mut client = initialized_client(&server).await;

    let mut args = HashMap::new();
    args.insert("name".to_string(), "serde".to_string());
    let result = client.get_prompt("analyze_crate", args).await;

    let text = result.first_message_text().expect("expected message text");
    assert!(text.contains("serde"));
    assert!(text.contains("Quality"));
    assert!(text.contains("Maintenance"));
}

#[tokio::test]
async fn prompt_analyze_crate_with_use_case() {
    let server = MockServer::start().await;
    let mut client = initialized_client(&server).await;

    let mut args = HashMap::new();
    args.insert("name".to_string(), "serde".to_string());
    args.insert("use_case".to_string(), "JSON serialization".to_string());
    let result = client.get_prompt("analyze_crate", args).await;

    let text = result.first_message_text().expect("expected message text");
    assert!(text.contains("serde"));
    assert!(text.contains("JSON serialization"));
}

#[tokio::test]
async fn prompt_compare_crates() {
    let server = MockServer::start().await;
    let mut client = initialized_client(&server).await;

    let mut args = HashMap::new();
    args.insert("crates".to_string(), "serde, bincode".to_string());
    args.insert("use_case".to_string(), "binary serialization".to_string());
    let result = client.get_prompt("compare_crates", args).await;

    let text = result.first_message_text().expect("expected message text");
    assert!(text.contains("serde"));
    assert!(text.contains("bincode"));
    assert!(text.contains("binary serialization"));
}
