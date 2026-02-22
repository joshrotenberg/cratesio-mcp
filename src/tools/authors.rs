//! Get crate authors tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for getting crate authors
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuthorsInput {
    /// Crate name
    name: String,
    /// Version (defaults to latest)
    #[serde(default)]
    version: Option<String>,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_crate_authors")
        .description(
            "Get the authors of a specific crate version. Authors are the people \
             listed in the Cargo.toml [package].authors field.",
        )
        .read_only()
        .idempotent()
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<AuthorsInput>| async move {
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

                let authors = state
                    .client
                    .crate_authors(&input.name, &version)
                    .await
                    .tool_context("Crates.io API error")?;

                let mut output = format!("# {} v{} - Authors\n\n", input.name, version);

                if authors.names.is_empty() {
                    output.push_str("No authors listed for this version.\n");
                } else {
                    for name in &authors.names {
                        output.push_str(&format!("- {}\n", name));
                    }
                }

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
