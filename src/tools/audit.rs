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
