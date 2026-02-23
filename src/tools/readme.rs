//! Get crate readme tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for getting a crate's README
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadmeInput {
    /// Crate name
    name: String,
    /// Version (defaults to latest)
    #[serde(default)]
    version: Option<String>,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_crate_readme")
        .description(
            "Get the README content for a crate version. Returns the rendered README \
             from the crate's published package. Defaults to the latest version.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<ReadmeInput>| async move {
                // If no version specified, get the latest
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

                let readme = state
                    .client
                    .crate_readme(&input.name, &version)
                    .await
                    .tool_context("Crates.io API error")?;

                if readme.trim().is_empty() {
                    Ok(CallToolResult::text(format!(
                        "No README found for {} v{}",
                        input.name, version
                    )))
                } else {
                    Ok(CallToolResult::text(format!(
                        "# {} v{} - README\n\n{}",
                        input.name, version, readme
                    )))
                }
            },
        )
        .build()
}
