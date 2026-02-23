use std::time::Duration;

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::CratesIoClient;

/// Create a client pointed at the mock server with no rate limiting.
fn test_client(base_url: &str) -> CratesIoClient {
    CratesIoClient::with_base_url("test-agent", Duration::from_millis(0), base_url).unwrap()
}

// ── get_crate ───────────────────────────────────────────────────────────────

/// Mock JSON derived from live `GET /api/v1/crates/tower-mcp` (2026-02-22).
/// Trimmed to the fields our `CrateResponse` / `Crate` / `Version` types
/// actually deserialize.
const GET_CRATE_JSON: &str = r#"{
    "crate": {
        "name": "tower-mcp",
        "updated_at": "2026-02-11T13:21:51.089324Z",
        "keywords": ["ai", "json-rpc", "llm", "mcp", "tower"],
        "categories": ["asynchronous", "network-programming"],
        "created_at": "2026-01-28T16:29:05.281129Z",
        "downloads": 1721,
        "recent_downloads": 1721,
        "max_version": "0.6.0",
        "max_stable_version": "0.6.0",
        "description": "Tower-native Model Context Protocol (MCP) implementation",
        "homepage": "https://github.com/joshrotenberg/tower-mcp",
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
            "license": "MIT OR Apache-2.0",
            "rust_version": "1.85"
        }
    ]
}"#;

#[tokio::test]
async fn get_crate_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(GET_CRATE_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let resp = client.get_crate("tower-mcp").await.unwrap();

    assert_eq!(resp.crate_data.name, "tower-mcp");
    assert_eq!(resp.crate_data.max_version, "0.6.0");
    assert_eq!(resp.crate_data.downloads, 1721);
    assert_eq!(
        resp.crate_data.description.as_deref(),
        Some("Tower-native Model Context Protocol (MCP) implementation")
    );
    assert_eq!(
        resp.crate_data.repository.as_deref(),
        Some("https://github.com/joshrotenberg/tower-mcp")
    );
    assert_eq!(resp.versions.len(), 2);
    assert_eq!(resp.versions[0].num, "0.6.0");
    assert!(!resp.versions[0].yanked);
    assert_eq!(resp.versions[0].downloads, 119);
    assert_eq!(resp.versions[1].num, "0.5.0");
}

#[tokio::test]
async fn get_crate_not_found() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/nonexistent-crate-xyz"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errors": [{"detail": "Not Found"}]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let err = client.get_crate("nonexistent-crate-xyz").await.unwrap_err();

    assert!(
        matches!(err, super::Error::NotFound(ref p) if p.contains("nonexistent-crate-xyz")),
        "expected NotFound, got: {err:?}"
    );
}

// ── crate_owners ────────────────────────────────────────────────────────────

/// Mock JSON derived from live `GET /api/v1/crates/tower-mcp/owners` (2026-02-22).
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

#[tokio::test]
async fn crate_owners_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/owners"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(OWNERS_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let owners = client.crate_owners("tower-mcp").await.unwrap();

    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].login, "joshrotenberg");
    assert_eq!(owners[0].name.as_deref(), Some("Josh Rotenberg"));
    assert_eq!(owners[0].kind.as_deref(), Some("user"));
    assert_eq!(owners[0].url, "https://github.com/joshrotenberg");
}

// ── summary ────────────────────────────────────────────────────────────────

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

#[tokio::test]
async fn summary_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/summary"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SUMMARY_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let summary = client.summary().await.unwrap();

    assert_eq!(summary.num_crates, 180000);
    assert_eq!(summary.num_downloads, 50000000000);
    assert_eq!(summary.new_crates.len(), 1);
    assert_eq!(summary.new_crates[0].name, "new-crate");
    assert_eq!(summary.most_downloaded.len(), 1);
    assert_eq!(summary.most_downloaded[0].name, "serde");
    assert_eq!(summary.popular_keywords.len(), 1);
    assert_eq!(summary.popular_keywords[0].keyword, "serde");
    assert_eq!(summary.popular_categories.len(), 1);
}

// ── crates (search) ────────────────────────────────────────────────────────

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
        },
        {
            "name": "rmcp",
            "max_version": "0.1.0",
            "description": "Rust MCP SDK",
            "downloads": 500,
            "created_at": "2026-01-01T00:00:00.000000Z",
            "updated_at": "2026-02-01T00:00:00.000000Z"
        }
    ],
    "meta": { "total": 2 }
}"#;

#[tokio::test]
async fn crates_search_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SEARCH_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let query = super::CratesQuery::builder().search("mcp").build();
    let page = client.crates(query).await.unwrap();

    assert_eq!(page.meta.total, 2);
    assert_eq!(page.crates.len(), 2);
    assert_eq!(page.crates[0].name, "tower-mcp");
    assert_eq!(page.crates[0].max_version, "0.6.0");
    assert_eq!(page.crates[0].downloads, 1721);
    assert_eq!(page.crates[1].name, "rmcp");
}

// ── crate_downloads ────────────────────────────────────────────────────────

const DOWNLOADS_JSON: &str = r#"{
    "version_downloads": [
        { "version": 100, "downloads": 50, "date": "2026-02-20" },
        { "version": 100, "downloads": 42, "date": "2026-02-21" },
        { "version": 101, "downloads": 10, "date": "2026-02-21" }
    ]
}"#;

#[tokio::test]
async fn crate_downloads_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/downloads"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(DOWNLOADS_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let downloads = client.crate_downloads("tower-mcp").await.unwrap();

    assert_eq!(downloads.version_downloads.len(), 3);
    assert_eq!(downloads.version_downloads[0].version, 100);
    assert_eq!(downloads.version_downloads[0].downloads, 50);
    assert_eq!(
        downloads.version_downloads[0].date.as_deref(),
        Some("2026-02-20")
    );
    assert_eq!(downloads.version_downloads[2].version, 101);
}

// ── crate_versions ─────────────────────────────────────────────────────────

const VERSIONS_PAGE_JSON: &str = r#"{
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
    ],
    "meta": { "total": 6 }
}"#;

#[tokio::test]
async fn crate_versions_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/versions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(VERSIONS_PAGE_JSON, "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let page = client
        .crate_versions("tower-mcp", None, None)
        .await
        .unwrap();

    assert_eq!(page.meta.total, 6);
    assert_eq!(page.versions.len(), 2);
    assert_eq!(page.versions[0].num, "0.6.0");
    assert_eq!(page.versions[0].downloads, 119);
    assert_eq!(page.versions[0].rust_version.as_deref(), Some("1.90"));
    assert_eq!(page.versions[1].num, "0.5.0");
    assert!(page.versions[1].rust_version.is_none());
}

// ── crate_version ──────────────────────────────────────────────────────────

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

#[tokio::test]
async fn crate_version_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/0.6.0"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(VERSION_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let version = client.crate_version("tower-mcp", "0.6.0").await.unwrap();

    assert_eq!(version.num, "0.6.0");
    assert!(!version.yanked);
    assert_eq!(version.downloads, 119);
    assert_eq!(version.license.as_deref(), Some("MIT OR Apache-2.0"));
    assert_eq!(version.rust_version.as_deref(), Some("1.90"));
}

// ── crate_dependencies ─────────────────────────────────────────────────────

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
            "crate_id": "serde",
            "req": "^1",
            "kind": "normal",
            "optional": true,
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

#[tokio::test]
async fn crate_dependencies_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/0.6.0/dependencies"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(DEPENDENCIES_JSON, "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let deps = client
        .crate_dependencies("tower-mcp", "0.6.0")
        .await
        .unwrap();

    assert_eq!(deps.len(), 3);
    assert_eq!(deps[0].crate_id, "tokio");
    assert_eq!(deps[0].req, "^1");
    assert_eq!(deps[0].kind, "normal");
    assert!(!deps[0].optional);
    assert!(deps[1].optional);
    assert_eq!(deps[2].crate_id, "wiremock");
    assert_eq!(deps[2].kind, "dev");
}

// ── crate_authors ──────────────────────────────────────────────────────────

const AUTHORS_JSON: &str = r#"{
    "meta": {
        "names": ["Josh Rotenberg <josh@example.com>"]
    }
}"#;

#[tokio::test]
async fn crate_authors_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/0.6.0/authors"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(AUTHORS_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let authors = client.crate_authors("tower-mcp", "0.6.0").await.unwrap();

    assert_eq!(authors.names.len(), 1);
    assert_eq!(authors.names[0], "Josh Rotenberg <josh@example.com>");
}

// ── crate_readme ───────────────────────────────────────────────────────────

#[tokio::test]
async fn crate_readme_returns_text() {
    let server = MockServer::start().await;
    let readme_text = "# tower-mcp\n\nA Tower-native MCP implementation.";

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/0.6.0/readme"))
        .respond_with(ResponseTemplate::new(200).set_body_string(readme_text))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let readme = client.crate_readme("tower-mcp", "0.6.0").await.unwrap();

    assert_eq!(readme, readme_text);
}

// ── crate_reverse_dependencies ─────────────────────────────────────────────

const REVERSE_DEPS_JSON: &str = r#"{
    "dependencies": [
        {
            "crate_id": "tower-mcp",
            "req": "^0.6",
            "kind": "normal",
            "optional": false,
            "version_id": 200
        },
        {
            "crate_id": "tower-mcp",
            "req": "^0.5",
            "kind": "normal",
            "optional": false,
            "version_id": 201
        }
    ],
    "versions": [
        { "id": 200, "crate": "cratesio-mcp", "num": "0.1.0" },
        { "id": 201, "crate": "my-other-app", "num": "0.3.0" }
    ],
    "meta": { "total": 2 }
}"#;

#[tokio::test]
async fn crate_reverse_dependencies_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/reverse_dependencies"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(REVERSE_DEPS_JSON, "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let rev = client
        .crate_reverse_dependencies("tower-mcp")
        .await
        .unwrap();

    assert_eq!(rev.meta.total, 2);
    assert_eq!(rev.dependencies.len(), 2);
    assert_eq!(rev.dependencies[0].crate_version.crate_name, "cratesio-mcp");
    assert_eq!(rev.dependencies[0].crate_version.num, "0.1.0");
    assert_eq!(rev.dependencies[0].dependency.req, "^0.6");
    assert_eq!(rev.dependencies[1].crate_version.crate_name, "my-other-app");
    assert_eq!(rev.dependencies[1].crate_version.num, "0.3.0");
}

// ── version_downloads ──────────────────────────────────────────────────────

const VERSION_DOWNLOADS_JSON: &str = r#"{
    "version_downloads": [
        { "version": 100, "downloads": 30, "date": "2026-02-20" },
        { "version": 100, "downloads": 25, "date": "2026-02-21" }
    ]
}"#;

#[tokio::test]
async fn version_downloads_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/tower-mcp/0.6.0/downloads"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(VERSION_DOWNLOADS_JSON, "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let downloads = client
        .version_downloads("tower-mcp", "0.6.0")
        .await
        .unwrap();

    assert_eq!(downloads.version_downloads.len(), 2);
    assert_eq!(downloads.version_downloads[0].downloads, 30);
    assert_eq!(downloads.version_downloads[1].downloads, 25);
}

// ── user ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn user_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/users/joshrotenberg"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
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

    let client = test_client(&server.uri());
    let user = client.user("joshrotenberg").await.unwrap();

    assert_eq!(user.login, "joshrotenberg");
    assert_eq!(user.name.as_deref(), Some("Josh Rotenberg"));
    assert_eq!(user.kind.as_deref(), Some("user"));
}

// ── user_stats ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn user_stats_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/users/12345/stats"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_downloads": 999999
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let stats = client.user_stats(12345).await.unwrap();

    assert_eq!(stats.total_downloads, 999999);
}

// ── team ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn team_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/teams/github:rust-lang:libs"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "team": {
                "login": "github:rust-lang:libs",
                "name": "Rust Libraries Team",
                "avatar": null,
                "url": "https://github.com/rust-lang"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let team = client.team("github:rust-lang:libs").await.unwrap();

    assert_eq!(team.login, "github:rust-lang:libs");
    assert_eq!(team.name.as_deref(), Some("Rust Libraries Team"));
}

// ── categories ─────────────────────────────────────────────────────────────

const CATEGORIES_JSON: &str = r#"{
    "categories": [
        {
            "category": "Asynchronous",
            "crates_cnt": 3000,
            "slug": "asynchronous",
            "description": "Crates for async programming"
        },
        {
            "category": "Web programming",
            "crates_cnt": 2500,
            "slug": "web-programming",
            "description": "Web frameworks and tools"
        }
    ],
    "meta": { "total": 75 }
}"#;

#[tokio::test]
async fn categories_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/categories"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CATEGORIES_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let page = client.categories(None, None).await.unwrap();

    assert_eq!(page.meta.total, 75);
    assert_eq!(page.categories.len(), 2);
    assert_eq!(page.categories[0].category, "Asynchronous");
    assert_eq!(page.categories[0].crates_cnt, 3000);
    assert_eq!(page.categories[0].slug.as_deref(), Some("asynchronous"));
    assert_eq!(page.categories[1].category, "Web programming");
}

// ── category ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn category_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/categories/asynchronous"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
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

    let client = test_client(&server.uri());
    let cat = client.category("asynchronous").await.unwrap();

    assert_eq!(cat.category, "Asynchronous");
    assert_eq!(cat.crates_cnt, 3000);
    assert_eq!(
        cat.description.as_deref(),
        Some("Crates for async programming")
    );
}

// ── keywords ───────────────────────────────────────────────────────────────

const KEYWORDS_JSON: &str = r#"{
    "keywords": [
        { "keyword": "serde", "crates_cnt": 5000 },
        { "keyword": "async", "crates_cnt": 3000 }
    ],
    "meta": { "total": 10000 }
}"#;

#[tokio::test]
async fn keywords_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/keywords"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(KEYWORDS_JSON, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let page = client.keywords(None, None).await.unwrap();

    assert_eq!(page.meta.total, 10000);
    assert_eq!(page.keywords.len(), 2);
    assert_eq!(page.keywords[0].keyword, "serde");
    assert_eq!(page.keywords[0].crates_cnt, 5000);
    assert_eq!(page.keywords[1].keyword, "async");
}

// ── keyword ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn keyword_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/keywords/serde"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "keyword": {
                "keyword": "serde",
                "crates_cnt": 5000
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let kw = client.keyword("serde").await.unwrap();

    assert_eq!(kw.keyword, "serde");
    assert_eq!(kw.crates_cnt, 5000);
}

// ── category_slugs ─────────────────────────────────────────────────────────

#[tokio::test]
async fn category_slugs_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/category_slugs"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "category_slugs": [
                { "id": "asynchronous", "slug": "asynchronous", "description": "Async crates" },
                { "id": "web-programming", "slug": "web-programming", "description": "Web crates" }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let slugs = client.category_slugs().await.unwrap();

    assert_eq!(slugs.len(), 2);
    assert_eq!(slugs[0].id, "asynchronous");
    assert_eq!(slugs[0].slug, "asynchronous");
    assert_eq!(slugs[0].description.as_deref(), Some("Async crates"));
    assert_eq!(slugs[1].slug, "web-programming");
}

// ── site_metadata ──────────────────────────────────────────────────────────

#[tokio::test]
async fn site_metadata_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/site_metadata"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "deployed_sha": "abc123def456",
            "commit": "abc123def456"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let meta = client.site_metadata().await.unwrap();

    assert_eq!(meta.deployed_sha.as_deref(), Some("abc123def456"));
    assert_eq!(meta.commit.as_deref(), Some("abc123def456"));
}

// ── error mapping ───────────────────────────────────────────────────────────

#[tokio::test]
async fn unauthorized_maps_to_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/me"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri()).with_auth("bad-token");
    let err = client.me().await.unwrap_err();

    assert!(
        matches!(err, super::Error::Unauthorized),
        "expected Unauthorized, got: {err:?}"
    );
}

#[tokio::test]
async fn auth_required_without_token() {
    let server = MockServer::start().await;
    // No mock needed -- the client should fail before making a request.

    let client = test_client(&server.uri()); // no .with_auth()
    let err = client.me().await.unwrap_err();

    assert!(
        matches!(err, super::Error::AuthRequired),
        "expected AuthRequired, got: {err:?}"
    );
}

#[tokio::test]
async fn rate_limited_maps_to_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/summary"))
        .respond_with(ResponseTemplate::new(429))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let err = client.summary().await.unwrap_err();

    assert!(
        matches!(err, super::Error::RateLimited),
        "expected RateLimited, got: {err:?}"
    );
}

#[tokio::test]
async fn forbidden_maps_to_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/crates/private-crate"))
        .respond_with(ResponseTemplate::new(403))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let err = client.get_crate("private-crate").await.unwrap_err();

    assert!(
        matches!(err, super::Error::PermissionDenied),
        "expected PermissionDenied, got: {err:?}"
    );
}

#[tokio::test]
async fn server_error_maps_to_api_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/summary"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let err = client.summary().await.unwrap_err();

    assert!(
        matches!(err, super::Error::Api { status: 500, .. }),
        "expected Api {{ status: 500 }}, got: {err:?}"
    );
}

// ── auth header ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn auth_header_sent_on_authenticated_request() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/me"))
        .and(header("Authorization", "my-secret-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "user": {
                "login": "testuser",
                "name": "Test User",
                "url": "https://github.com/testuser",
                "avatar": null,
                "kind": "user"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri()).with_auth("my-secret-token");
    let user = client.me().await.unwrap();

    assert_eq!(user.login, "testuser");
}
