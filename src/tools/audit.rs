//! Dependency security audit tool via OSV.dev

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::client::osv::OsvVulnerability;
use crate::state::AppState;

/// Input for auditing dependencies
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuditInput {
    /// Crate name to audit
    name: String,
    /// Version to audit (default: latest)
    #[serde(default)]
    version: Option<String>,
    /// Include dev dependencies in audit
    #[serde(default)]
    include_dev: bool,
}

/// A vulnerability finding associated with a dependency.
struct Finding {
    dep_name: String,
    vuln: OsvVulnerability,
}

fn format_findings(
    crate_name: &str,
    version: &str,
    findings: &[Finding],
    deps_checked: usize,
) -> String {
    let mut output = format!("# Security Audit: {} v{}\n\n", crate_name, version);

    if findings.is_empty() {
        output.push_str("No known vulnerabilities found.\n\n");
    } else {
        output.push_str("## Vulnerabilities Found\n\n");
        for f in findings {
            output.push_str(&format!("### {} -- {}\n\n", f.dep_name, f.vuln.id));

            if let Some(summary) = &f.vuln.summary {
                output.push_str(&format!("- **Summary**: {}\n", summary));
            }

            // Show CVSS severity if available
            if let Some(severity) = &f.vuln.severity
                && let Some(s) = severity.first()
            {
                output.push_str(&format!(
                    "- **Severity**: {} ({})\n",
                    s.severity_type, s.score
                ));
            }

            // Show fixed version if available
            if let Some(affected) = &f.vuln.affected {
                for a in affected {
                    if let Some(ranges) = &a.ranges {
                        for range in ranges {
                            for event in &range.events {
                                if let Some(fixed) = &event.fixed {
                                    output.push_str(&format!("- **Fixed in**: {}\n", fixed));
                                }
                            }
                        }
                    }
                }
            }

            // Show first advisory reference
            if let Some(refs) = &f.vuln.references {
                if let Some(r) = refs.iter().find(|r| r.ref_type == "ADVISORY") {
                    output.push_str(&format!("- **Advisory**: {}\n", r.url));
                } else if let Some(r) = refs.first() {
                    output.push_str(&format!("- **Reference**: {}\n", r.url));
                }
            }

            output.push('\n');
        }
    }

    // Summary
    let affected_deps: Vec<&str> = {
        let mut names: Vec<&str> = findings.iter().map(|f| f.dep_name.as_str()).collect();
        names.sort();
        names.dedup();
        names
    };

    output.push_str("## Summary\n\n");
    output.push_str(&format!("- **Dependencies checked**: {}\n", deps_checked));
    output.push_str(&format!(
        "- **Vulnerabilities found**: {}\n",
        findings.len()
    ));
    output.push_str(&format!(
        "- **Affected dependencies**: {}\n",
        affected_deps.len()
    ));

    output
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("audit_dependencies")
        .title("Audit Dependencies")
        .description(
            "Check a crate's dependencies against the OSV.dev vulnerability database \
             (RustSec + GHSA + NVD). Returns known vulnerabilities for each dependency.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<AuditInput>| async move {
                // Resolve crate version
                let crate_response = state
                    .client
                    .get_crate(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                let version = input
                    .version
                    .as_deref()
                    .unwrap_or(&crate_response.crate_data.max_version);

                // Fetch dependencies
                let deps = state
                    .client
                    .crate_dependencies(&input.name, version)
                    .await
                    .tool_context("Crates.io API error")?;

                // Filter out dev deps unless requested
                let deps_to_check: Vec<_> = deps
                    .iter()
                    .filter(|d| input.include_dev || d.kind != "dev")
                    .collect();

                let deps_checked = deps_to_check.len();
                let mut findings = Vec::new();

                // Check the crate itself
                let self_resp = state
                    .osv_client
                    .query_package_any(&input.name)
                    .await
                    .tool_context("OSV.dev API error")?;

                if let Some(vulns) = self_resp.vulns {
                    for vuln in vulns {
                        findings.push(Finding {
                            dep_name: input.name.clone(),
                            vuln,
                        });
                    }
                }

                // Check each dependency
                for dep in &deps_to_check {
                    let resp = state
                        .osv_client
                        .query_package_any(&dep.crate_id)
                        .await
                        .tool_context("OSV.dev API error")?;

                    if let Some(vulns) = resp.vulns {
                        for vuln in vulns {
                            findings.push(Finding {
                                dep_name: dep.crate_id.clone(),
                                vuln,
                            });
                        }
                    }
                }

                let output = format_findings(&input.name, version, &findings, deps_checked);
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
            client: CratesIoClient::with_base_url(
                "test",
                Duration::from_millis(0),
                Duration::from_secs(30),
                crates_url,
            )
            .unwrap(),
            docsrs_client: DocsRsClient::with_base_url("test", Duration::from_secs(30), crates_url)
                .unwrap(),
            osv_client: OsvClient::with_base_url("test", Duration::from_secs(30), osv_url).unwrap(),
            docs_cache: DocsCache::new(10, Duration::from_secs(3600)),
            recent_searches: RwLock::new(Vec::new()),
        })
    }

    #[tokio::test]
    async fn audit_vulnerability_found() {
        let crates_server = MockServer::start().await;
        let osv_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/vuln-crate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "vuln-crate",
                    "max_version": "0.1.0",
                    "description": "Vulnerable crate",
                    "downloads": 100,
                    "created_at": "2026-01-01T00:00:00.000000Z",
                    "updated_at": "2026-01-01T00:00:00.000000Z"
                },
                "versions": [{"num": "0.1.0", "yanked": false, "created_at": "2026-01-01T00:00:00.000000Z", "downloads": 100}]
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

        Mock::given(method("POST"))
            .and(path("/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "vulns": [
                    {
                        "id": "RUSTSEC-2024-0001",
                        "summary": "Use-after-free in vuln-crate",
                        "references": [{"type": "ADVISORY", "url": "https://rustsec.org/advisories/RUSTSEC-2024-0001.html"}]
                    }
                ]
            })))
            .mount(&osv_server)
            .await;

        let state = test_state(&crates_server.uri(), &osv_server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "vuln-crate"})).await;

        let text = result.all_text();
        assert!(!result.is_error);
        assert!(
            text.contains("RUSTSEC-2024-0001"),
            "advisory ID should appear in output, got: {text}"
        );
        assert!(text.contains("Vulnerabilities Found"));
    }

    #[tokio::test]
    async fn audit_include_dev_deps() {
        let crates_server = MockServer::start().await;
        let osv_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/my-crate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "my-crate",
                    "max_version": "1.0.0",
                    "description": "Test crate",
                    "downloads": 100,
                    "created_at": "2026-01-01T00:00:00.000000Z",
                    "updated_at": "2026-01-01T00:00:00.000000Z"
                },
                "versions": [{"num": "1.0.0", "yanked": false, "created_at": "2026-01-01T00:00:00.000000Z", "downloads": 100}]
            })))
            .mount(&crates_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/my-crate/1.0.0/dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": [
                    {"crate_id": "normal-dep", "req": "^1", "kind": "normal", "optional": false, "version_id": 1},
                    {"crate_id": "dev-dep", "req": "^1", "kind": "dev", "optional": false, "version_id": 2}
                ]
            })))
            .mount(&crates_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "vulns": []
            })))
            .mount(&osv_server)
            .await;

        let state = test_state(&crates_server.uri(), &osv_server.uri());

        // Default (include_dev=false): only normal dep counted
        let tool = super::build(Arc::clone(&state));
        let result = tool
            .call(serde_json::json!({"name": "my-crate", "include_dev": false}))
            .await;
        assert!(
            result.all_text().contains("Dependencies checked**: 1"),
            "without include_dev only normal dep should be counted"
        );

        // include_dev=true: normal + dev dep both counted
        let tool2 = super::build(state);
        let result2 = tool2
            .call(serde_json::json!({"name": "my-crate", "include_dev": true}))
            .await;
        assert!(
            result2.all_text().contains("Dependencies checked**: 2"),
            "with include_dev both deps should be counted"
        );
    }

    #[tokio::test]
    async fn audit_explicit_version_override() {
        let crates_server = MockServer::start().await;
        let osv_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/my-crate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "my-crate",
                    "max_version": "2.0.0",
                    "description": "Test crate",
                    "downloads": 100,
                    "created_at": "2026-01-01T00:00:00.000000Z",
                    "updated_at": "2026-01-01T00:00:00.000000Z"
                },
                "versions": [
                    {"num": "2.0.0", "yanked": false, "created_at": "2026-06-01T00:00:00.000000Z", "downloads": 50},
                    {"num": "1.0.0", "yanked": false, "created_at": "2026-01-01T00:00:00.000000Z", "downloads": 50}
                ]
            })))
            .mount(&crates_server)
            .await;

        // Only the 1.0.0 endpoint is mocked -- if the tool incorrectly used 2.0.0 it would 404
        Mock::given(method("GET"))
            .and(path("/crates/my-crate/1.0.0/dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": []
            })))
            .mount(&crates_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "vulns": []
            })))
            .mount(&osv_server)
            .await;

        let state = test_state(&crates_server.uri(), &osv_server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"name": "my-crate", "version": "1.0.0"}))
            .await;

        let text = result.all_text();
        assert!(!result.is_error);
        assert!(
            text.contains("my-crate v1.0.0"),
            "audit header should use the overridden version 1.0.0, got: {text}"
        );
    }

    #[tokio::test]
    async fn audit_osv_error_surfaced() {
        let crates_server = MockServer::start().await;
        let osv_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/my-crate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "my-crate",
                    "max_version": "1.0.0",
                    "description": "Test crate",
                    "downloads": 100,
                    "created_at": "2026-01-01T00:00:00.000000Z",
                    "updated_at": "2026-01-01T00:00:00.000000Z"
                },
                "versions": [{"num": "1.0.0", "yanked": false, "created_at": "2026-01-01T00:00:00.000000Z", "downloads": 100}]
            })))
            .mount(&crates_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/my-crate/1.0.0/dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": []
            })))
            .mount(&crates_server)
            .await;

        // OSV returns a server error -- should surface as a tool error
        Mock::given(method("POST"))
            .and(path("/query"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&osv_server)
            .await;

        let state = test_state(&crates_server.uri(), &osv_server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "my-crate"})).await;

        assert!(
            result.is_error,
            "OSV API error should surface as a tool error"
        );
    }

    #[test]
    fn input_deserializes_without_version_key() {
        let input: super::AuditInput =
            serde_json::from_value(serde_json::json!({"name": "serde"})).unwrap();
        assert!(input.version.is_none());
    }
}
