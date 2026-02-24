//! Crate health check composite tool

use std::sync::Arc;

use chrono::Utc;
use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, format_number};

/// Input for crate health check
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HealthCheckInput {
    /// Crate name to evaluate
    name: String,
    /// Version to check (default: latest)
    version: Option<String>,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("crate_health_check")
        .description(
            "Comprehensive health check for a crate. Combines multiple API calls into a single \
             report covering maturity, adoption, maintenance, security, compatibility, and \
             dependency weight. Answers: \"should I use this crate?\"",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<HealthCheckInput>| async move {
                // 1. Get crate info (basic metadata + version list)
                let crate_response = state
                    .client
                    .get_crate(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                let crate_data = &crate_response.crate_data;
                let version = input
                    .version
                    .as_deref()
                    .unwrap_or(&crate_data.max_version)
                    .to_string();

                // 2. Get version details (license, MSRV)
                let version_detail = state
                    .client
                    .crate_version(&input.name, &version)
                    .await
                    .tool_context("Crates.io API error")?;

                // 3. Get dependencies
                let deps = state
                    .client
                    .crate_dependencies(&input.name, &version)
                    .await
                    .tool_context("Crates.io API error")?;

                let normal_deps: Vec<_> = deps.iter().filter(|d| d.kind == "normal").collect();
                let normal_required: Vec<_> = normal_deps.iter().filter(|d| !d.optional).collect();
                let normal_optional: Vec<_> = normal_deps.iter().filter(|d| d.optional).collect();
                let build_deps: Vec<_> = deps.iter().filter(|d| d.kind == "build").collect();

                // 4. Get reverse dependencies (adoption signal)
                let rev_deps = state
                    .client
                    .crate_reverse_dependencies(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                // 5. Check vulnerabilities via OSV
                let self_vulns = state
                    .osv_client
                    .query_package_any(&input.name)
                    .await
                    .tool_context("OSV.dev API error")?;

                let vuln_count = self_vulns.vulns.as_ref().map_or(0, |v| v.len());

                // -- Compute derived metrics --

                let now = Utc::now();
                let age_days = (now - crate_data.created_at).num_days();
                let days_since_update = (now - crate_data.updated_at).num_days();
                let total_versions = crate_response.versions.len();

                // Release cadence: average days between releases
                let cadence = if total_versions > 1 {
                    let first = crate_response
                        .versions
                        .last()
                        .map(|v| v.created_at)
                        .unwrap_or(crate_data.created_at);
                    let latest = crate_response
                        .versions
                        .first()
                        .map(|v| v.created_at)
                        .unwrap_or(crate_data.updated_at);
                    let span = (latest - first).num_days();
                    Some(span / (total_versions as i64 - 1))
                } else {
                    None
                };

                let yanked_count = crate_response.versions.iter().filter(|v| v.yanked).count();

                // -- Format output --

                let mut output = format!("# Health Check: {} v{}\n\n", input.name, version);

                // Description
                if let Some(desc) = &crate_data.description {
                    output.push_str(&format!("> {}\n\n", desc));
                }

                // Maturity
                output.push_str("## Maturity\n\n");
                let age_str = if age_days > 365 {
                    format!("{:.1} years", age_days as f64 / 365.0)
                } else {
                    format!("{} days", age_days)
                };
                output.push_str(&format!("- **Age**: {}\n", age_str));
                output.push_str(&format!("- **Total versions**: {}\n", total_versions));
                if let Some(c) = cadence {
                    output.push_str(&format!("- **Avg release cadence**: {} days\n", c));
                }
                if yanked_count > 0 {
                    output.push_str(&format!("- **Yanked versions**: {}\n", yanked_count));
                }

                // Adoption
                output.push_str("\n## Adoption\n\n");
                output.push_str(&format!(
                    "- **Total downloads**: {}\n",
                    format_number(crate_data.downloads)
                ));
                if let Some(recent) = crate_data.recent_downloads {
                    output.push_str(&format!(
                        "- **Recent downloads**: {}\n",
                        format_number(recent)
                    ));
                }
                output.push_str(&format!(
                    "- **Reverse dependencies**: {}\n",
                    rev_deps.meta.total
                ));

                // Maintenance
                output.push_str("\n## Maintenance\n\n");
                let freshness = if days_since_update <= 30 {
                    "Active (updated within 30 days)"
                } else if days_since_update <= 90 {
                    "Recent (updated within 90 days)"
                } else if days_since_update <= 365 {
                    "Aging (no update in 3-12 months)"
                } else {
                    "Stale (no update in over a year)"
                };
                output.push_str(&format!("- **Status**: {}\n", freshness));
                output.push_str(&format!(
                    "- **Last updated**: {} ({} days ago)\n",
                    crate_data.updated_at.date_naive(),
                    days_since_update
                ));

                // Security
                output.push_str("\n## Security\n\n");
                if vuln_count == 0 {
                    output.push_str("- **Known vulnerabilities**: None\n");
                } else {
                    output.push_str(&format!(
                        "- **Known vulnerabilities**: {} (run `audit_dependencies` for details)\n",
                        vuln_count
                    ));
                }

                // Compatibility
                output.push_str("\n## Compatibility\n\n");
                output.push_str(&format!(
                    "- **License**: {}\n",
                    version_detail.license.as_deref().unwrap_or("Not specified")
                ));
                output.push_str(&format!(
                    "- **MSRV**: {}\n",
                    version_detail
                        .rust_version
                        .as_deref()
                        .unwrap_or("Not specified")
                ));

                // Dependency weight
                output.push_str("\n## Dependency Weight\n\n");
                output.push_str(&format!(
                    "- **Required dependencies**: {}\n",
                    normal_required.len()
                ));
                if !normal_optional.is_empty() {
                    output.push_str(&format!(
                        "- **Optional dependencies**: {}\n",
                        normal_optional.len()
                    ));
                }
                if !build_deps.is_empty() {
                    output.push_str(&format!("- **Build dependencies**: {}\n", build_deps.len()));
                }

                // Links
                output.push_str("\n## Links\n\n");
                if let Some(repo) = &crate_data.repository {
                    output.push_str(&format!("- **Repository**: {}\n", repo));
                }
                if let Some(docs) = &crate_data.documentation {
                    output.push_str(&format!("- **Documentation**: {}\n", docs));
                }
                if let Some(home) = &crate_data.homepage {
                    output.push_str(&format!("- **Homepage**: {}\n", home));
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

    fn test_state(crates_url: &str, osv_url: &str) -> Arc<AppState> {
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
    async fn health_check_basic() {
        let crates_server = MockServer::start().await;
        let osv_server = MockServer::start().await;

        // Crate info
        Mock::given(method("GET"))
            .and(path("/crates/my-crate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "my-crate",
                    "max_version": "1.0.0",
                    "description": "A test crate",
                    "downloads": 50000,
                    "recent_downloads": 5000,
                    "created_at": "2024-01-01T00:00:00.000000Z",
                    "updated_at": "2026-02-01T00:00:00.000000Z",
                    "repository": "https://github.com/test/my-crate",
                    "documentation": "https://docs.rs/my-crate"
                },
                "versions": [
                    {"num": "1.0.0", "yanked": false, "created_at": "2026-02-01T00:00:00.000000Z", "downloads": 3000, "license": "MIT"},
                    {"num": "0.9.0", "yanked": false, "created_at": "2025-06-01T00:00:00.000000Z", "downloads": 20000},
                    {"num": "0.1.0", "yanked": false, "created_at": "2024-01-01T00:00:00.000000Z", "downloads": 27000}
                ]
            })))
            .mount(&crates_server)
            .await;

        // Version detail
        Mock::given(method("GET"))
            .and(path("/crates/my-crate/1.0.0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "version": {
                    "num": "1.0.0",
                    "yanked": false,
                    "created_at": "2026-02-01T00:00:00.000000Z",
                    "downloads": 3000,
                    "license": "MIT OR Apache-2.0",
                    "rust_version": "1.75"
                }
            })))
            .mount(&crates_server)
            .await;

        // Dependencies
        Mock::given(method("GET"))
            .and(path("/crates/my-crate/1.0.0/dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": [
                    {"crate_id": "serde", "req": "^1", "kind": "normal", "optional": false, "version_id": 1},
                    {"crate_id": "tokio", "req": "^1", "kind": "normal", "optional": true, "version_id": 2}
                ]
            })))
            .mount(&crates_server)
            .await;

        // Reverse deps
        Mock::given(method("GET"))
            .and(path("/crates/my-crate/reverse_dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": [],
                "versions": [],
                "meta": {"total": 42}
            })))
            .mount(&crates_server)
            .await;

        // OSV: no vulnerabilities
        Mock::given(method("POST"))
            .and(path("/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "vulns": []
            })))
            .mount(&osv_server)
            .await;

        let state = test_state(&crates_server.uri(), &osv_server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "my-crate"})).await;

        let text = result.all_text();
        assert!(text.contains("Health Check: my-crate v1.0.0"));
        assert!(text.contains("A test crate"));
        // Maturity
        assert!(text.contains("Total versions"));
        assert!(text.contains("3"));
        assert!(text.contains("Avg release cadence"));
        // Adoption
        assert!(text.contains("50.0K"));
        assert!(text.contains("5.0K"));
        assert!(text.contains("42"));
        // Maintenance
        assert!(text.contains("Last updated"));
        // Security
        assert!(text.contains("None"));
        // Compatibility
        assert!(text.contains("MIT OR Apache-2.0"));
        assert!(text.contains("1.75"));
        // Dependency weight
        assert!(text.contains("Required dependencies"));
        assert!(text.contains("Optional dependencies"));
        // Links
        assert!(text.contains("github.com/test/my-crate"));
        assert!(text.contains("docs.rs/my-crate"));
    }

    #[tokio::test]
    async fn health_check_with_vulnerabilities() {
        let crates_server = MockServer::start().await;
        let osv_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/vuln-crate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "vuln-crate",
                    "max_version": "0.1.0",
                    "description": "Has vulns",
                    "downloads": 100,
                    "created_at": "2025-01-01T00:00:00.000000Z",
                    "updated_at": "2025-01-01T00:00:00.000000Z"
                },
                "versions": [
                    {"num": "0.1.0", "yanked": false, "created_at": "2025-01-01T00:00:00.000000Z", "downloads": 100}
                ]
            })))
            .mount(&crates_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/vuln-crate/0.1.0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "version": {
                    "num": "0.1.0",
                    "yanked": false,
                    "created_at": "2025-01-01T00:00:00.000000Z",
                    "downloads": 100
                }
            })))
            .mount(&crates_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/vuln-crate/0.1.0/dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": []
            })))
            .mount(&crates_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/vuln-crate/reverse_dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": [],
                "versions": [],
                "meta": {"total": 0}
            })))
            .mount(&crates_server)
            .await;

        // OSV: has vulnerabilities
        Mock::given(method("POST"))
            .and(path("/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "vulns": [
                    {"id": "RUSTSEC-2025-0001", "summary": "Memory safety issue"},
                    {"id": "GHSA-xxxx-yyyy", "summary": "Another issue"}
                ]
            })))
            .mount(&osv_server)
            .await;

        let state = test_state(&crates_server.uri(), &osv_server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "vuln-crate"})).await;

        let text = result.all_text();
        assert!(text.contains("Health Check: vuln-crate"));
        assert!(text.contains("2"));
        assert!(text.contains("audit_dependencies"));
        // Stale crate
        assert!(text.contains("Stale") || text.contains("Aging"));
    }
}
