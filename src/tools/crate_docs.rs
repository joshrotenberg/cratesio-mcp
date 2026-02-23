//! Browse crate documentation structure from docs.rs.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::docs::format;
use crate::state::AppState;

/// Input for browsing crate documentation
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCrateDocsInput {
    /// Crate name (e.g. "serde", "tokio")
    name: String,
    /// Version (default: "latest")
    #[serde(default = "default_version")]
    version: String,
    /// Module path to browse (e.g. "de", "io::util"). Omit for crate root.
    module_path: Option<String>,
}

fn default_version() -> String {
    "latest".to_string()
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_crate_docs")
        .description(
            "Browse a crate's documentation structure from docs.rs. \
             Lists modules, structs, traits, functions, and other items \
             in a module with brief descriptions. Use module_path to \
             navigate into sub-modules.",
        )
        .read_only()
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<GetCrateDocsInput>| async move {
                let krate = state
                    .docs_cache
                    .get_or_fetch(&state.docsrs_client, &input.name, &input.version)
                    .await
                    .tool_context("docs.rs fetch error")?;

                let module_id = if let Some(ref path) = input.module_path {
                    format::resolve_module_path(&krate, path).ok_or_else(|| {
                        tower_mcp::ToolError::new(format!(
                            "Module '{}' not found in {} v{}",
                            path, input.name, input.version
                        ))
                    })?
                } else {
                    krate.root
                };

                let output = format::format_module_listing(&krate, &module_id);
                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
