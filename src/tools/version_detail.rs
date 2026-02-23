//! Get specific crate version detail tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, format_number};

/// Input for getting a specific version
#[derive(Debug, Deserialize, JsonSchema)]
pub struct VersionDetailInput {
    /// Crate name
    name: String,
    /// Version string (e.g. "1.0.0")
    version: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_crate_version")
        .description(
            "Get detailed metadata for a specific crate version including \
             license, MSRV, download count, and yanked status.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<VersionDetailInput>| async move {
                let v = state
                    .client
                    .crate_version(&input.name, &input.version)
                    .await
                    .tool_context("Crates.io API error")?;

                let yanked = if v.yanked { " [YANKED]" } else { "" };
                let mut output = format!("# {} v{}{}\n\n", input.name, v.num, yanked);

                output.push_str(&format!("- **Released:** {}\n", v.created_at.date_naive()));
                output.push_str(&format!(
                    "- **Downloads:** {}\n",
                    format_number(v.downloads)
                ));

                if let Some(license) = &v.license {
                    output.push_str(&format!("- **License:** {}\n", license));
                }
                if let Some(msrv) = &v.rust_version {
                    output.push_str(&format!("- **MSRV:** {}\n", msrv));
                }

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
