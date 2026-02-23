//! Get documentation for a specific item from docs.rs.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::docs::format;
use crate::state::AppState;

/// Input for getting item documentation
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDocItemInput {
    /// Crate name (e.g. "serde", "tokio")
    name: String,
    /// Version (default: "latest")
    #[serde(default = "default_version")]
    version: String,
    /// Item path (e.g. "McpRouter", "de::from_str", "Serialize")
    item_path: String,
}

fn default_version() -> String {
    "latest".to_string()
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_doc_item")
        .description(
            "Get full documentation for a specific item (function, struct, trait, etc.) \
             from docs.rs. Includes the item's signature, doc comments, and for structs, \
             the list of public methods.",
        )
        .read_only()
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<GetDocItemInput>| async move {
                let krate = state
                    .docs_cache
                    .get_or_fetch(&state.docsrs_client, &input.name, &input.version)
                    .await
                    .tool_context("docs.rs fetch error")?;

                let item =
                    format::resolve_item_path(&krate, &input.item_path).ok_or_else(|| {
                        tower_mcp::ToolError::new(format!(
                            "Item '{}' not found in {} v{}",
                            input.item_path, input.name, input.version
                        ))
                    })?;

                let output = format::format_item_detail(&krate, item);
                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
