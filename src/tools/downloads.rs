//! Get downloads tool

use std::collections::HashMap;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, format_number};

/// Input for getting downloads data
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadsInput {
    /// Crate name
    name: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_downloads")
        .description(
            "Get download statistics for a crate including total downloads \
             and recent download trends.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<DownloadsInput>| async move {
                let response = state
                    .client
                    .crate_downloads(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                let mut output = format!("# {} - Download Statistics\n\n", input.name);

                // Get recent downloads (last 90 days from version_downloads)
                let total: u64 = response.version_downloads.iter().map(|v| v.downloads).sum();
                output.push_str(&format!(
                    "**Recent downloads (90 days):** {}\n\n",
                    format_number(total)
                ));

                // Show per-version breakdown
                output.push_str("## By Version\n\n");
                let mut version_totals: HashMap<u64, u64> = HashMap::new();
                for vd in &response.version_downloads {
                    *version_totals.entry(vd.version).or_default() += vd.downloads;
                }

                let mut versions: Vec<_> = version_totals.iter().collect();
                versions.sort_by(|a, b| b.1.cmp(a.1));

                for (version_id, downloads) in versions.iter().take(10) {
                    output.push_str(&format!(
                        "- Version {}: {}\n",
                        version_id,
                        format_number(**downloads)
                    ));
                }

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
