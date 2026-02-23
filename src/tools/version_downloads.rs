//! Get per-version download statistics tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, format_number};

/// Input for getting per-version downloads
#[derive(Debug, Deserialize, JsonSchema)]
pub struct VersionDownloadsInput {
    /// Crate name
    name: String,
    /// Version (defaults to latest)
    #[serde(default)]
    version: Option<String>,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_version_downloads")
        .description(
            "Get daily download statistics for a specific crate version. \
             Shows the download trend over the last 90 days for that version.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>,
             Json(input): Json<VersionDownloadsInput>| async move {
                let version = match input.version {
                    Some(v) => v,
                    None => {
                        let crate_info = state
                            .client
                            .get_crate(&input.name)
                            .await
                            .tool_context("Crates.io API error")?;
                        crate_info.crate_data.max_version.clone()
                    }
                };

                let response = state
                    .client
                    .version_downloads(&input.name, &version)
                    .await
                    .tool_context("Crates.io API error")?;

                let total: u64 = response.version_downloads.iter().map(|v| v.downloads).sum();

                let mut output = format!(
                    "# {} v{} - Download Statistics\n\n\
                     **Total (last 90 days):** {}\n\n",
                    input.name,
                    version,
                    format_number(total)
                );

                // Show daily data, most recent first
                let mut entries: Vec<_> = response
                    .version_downloads
                    .iter()
                    .filter(|vd| vd.downloads > 0)
                    .collect();
                entries.sort_by(|a, b| b.date.cmp(&a.date));

                if !entries.is_empty() {
                    output.push_str("## Daily Downloads\n\n");
                    output.push_str("| Date | Downloads |\n");
                    output.push_str("|------|----------|\n");
                    for vd in entries.iter().take(30) {
                        let date = vd.date.as_deref().unwrap_or("unknown");
                        output.push_str(&format!(
                            "| {} | {} |\n",
                            date,
                            format_number(vd.downloads)
                        ));
                    }
                }

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
