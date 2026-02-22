//! Get crate versions tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, format_number};

/// Input for getting versions
#[derive(Debug, Deserialize, JsonSchema)]
pub struct VersionsInput {
    /// Crate name
    name: String,
    /// Include yanked versions
    #[serde(default)]
    include_yanked: bool,
    /// Maximum number of versions (default: 10)
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    10
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_crate_versions")
        .description(
            "Get version history for a crate including version numbers, release dates, \
             download counts, and yanked status.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<VersionsInput>| async move {
                let response = state
                    .client
                    .get_crate(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                let versions: Vec<_> = response
                    .versions
                    .iter()
                    .filter(|v| input.include_yanked || !v.yanked)
                    .take(input.limit)
                    .collect();

                let mut output = format!("# {} - Version History\n\n", input.name);
                output.push_str(&format!("Showing {} versions:\n\n", versions.len()));

                for v in versions {
                    let yanked = if v.yanked { " [YANKED]" } else { "" };
                    output.push_str(&format!("## v{}{}\n", v.num, yanked));
                    output.push_str(&format!("- Released: {}\n", v.created_at.date_naive()));
                    output.push_str(&format!("- Downloads: {}\n", format_number(v.downloads)));
                    if let Some(license) = &v.license {
                        output.push_str(&format!("- License: {}\n", license));
                    }
                    if let Some(msrv) = &v.rust_version {
                        output.push_str(&format!("- MSRV: {}\n", msrv));
                    }
                    output.push('\n');
                }

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
