//! Compare crates tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, format_number};

/// Input for comparing crates
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompareInput {
    /// Comma-separated list of crate names to compare (2-5 crates)
    crates: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("compare_crates")
        .description(
            "Compare two or more crates side by side. Returns a structured comparison of \
             downloads, versions, dependencies, reverse dependencies, and freshness.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<CompareInput>| async move {
                let names: Vec<&str> = input.crates.split(',').map(|s| s.trim()).collect();

                if names.len() < 2 {
                    return Ok(CallToolResult::text(
                        "Please provide at least 2 crate names separated by commas.",
                    ));
                }
                if names.len() > 5 {
                    return Ok(CallToolResult::text(
                        "Please provide at most 5 crate names to compare.",
                    ));
                }

                let mut output = format!("# Crate Comparison: {}\n\n", names.join(" vs "));

                // Table header
                output.push_str("| | ");
                for name in &names {
                    output.push_str(&format!("**{}** | ", name));
                }
                output.push('\n');

                output.push_str("|---|");
                for _ in &names {
                    output.push_str("---|");
                }
                output.push('\n');

                // Gather data for each crate
                let mut versions_row = vec![];
                let mut downloads_row = vec![];
                let mut recent_row = vec![];
                let mut deps_row = vec![];
                let mut rev_deps_row = vec![];
                let mut last_release_row = vec![];
                let mut license_row = vec![];
                let mut msrv_row = vec![];
                let mut description_row = vec![];

                for name in &names {
                    let info = state.client.get_crate(name).await;
                    let rev_deps = state.client.crate_reverse_dependencies(name).await;

                    match info {
                        Ok(resp) => {
                            let c = &resp.crate_data;
                            versions_row.push(c.max_version.clone());
                            downloads_row.push(format_number(c.downloads));
                            recent_row.push(
                                c.recent_downloads
                                    .map(format_number)
                                    .unwrap_or_else(|| "-".to_string()),
                            );
                            last_release_row.push(c.updated_at.date_naive().to_string());
                            description_row
                                .push(c.description.clone().unwrap_or_else(|| "-".to_string()));

                            // Get deps and version details from the latest version
                            let version = &c.max_version;
                            match state.client.crate_dependencies(name, version).await {
                                Ok(deps) => {
                                    let normal: Vec<_> = deps
                                        .iter()
                                        .filter(|d| d.kind == "normal" && !d.optional)
                                        .collect();
                                    deps_row.push(format!("{}", normal.len()));
                                }
                                Err(_) => deps_row.push("-".to_string()),
                            }

                            match state.client.crate_version(name, version).await {
                                Ok(v) => {
                                    license_row.push(v.license.unwrap_or_else(|| "-".to_string()));
                                    msrv_row
                                        .push(v.rust_version.unwrap_or_else(|| "-".to_string()));
                                }
                                Err(_) => {
                                    license_row.push("-".to_string());
                                    msrv_row.push("-".to_string());
                                }
                            }
                        }
                        Err(e) => {
                            let err = format!("error: {}", e);
                            versions_row.push(err.clone());
                            downloads_row.push(err.clone());
                            recent_row.push(err.clone());
                            deps_row.push(err.clone());
                            last_release_row.push(err.clone());
                            license_row.push(err.clone());
                            msrv_row.push(err.clone());
                            description_row.push(err);
                        }
                    }

                    match rev_deps {
                        Ok(rd) => rev_deps_row.push(format!("{}", rd.meta.total)),
                        Err(_) => rev_deps_row.push("-".to_string()),
                    }
                }

                // Build table rows
                let rows = [
                    ("Description", &description_row),
                    ("Latest Version", &versions_row),
                    ("Total Downloads", &downloads_row),
                    ("Recent Downloads", &recent_row),
                    ("Direct Deps", &deps_row),
                    ("Reverse Deps", &rev_deps_row),
                    ("Last Release", &last_release_row),
                    ("License", &license_row),
                    ("MSRV", &msrv_row),
                ];

                for (label, values) in &rows {
                    output.push_str(&format!("| {} | ", label));
                    for val in *values {
                        output.push_str(&format!("{} | ", val));
                    }
                    output.push('\n');
                }

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
