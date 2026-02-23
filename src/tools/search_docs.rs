//! Search for items within a crate's documentation.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::docs::format;
use crate::state::AppState;

/// Input for searching crate documentation
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchDocsInput {
    /// Crate name (e.g. "serde", "tokio")
    name: String,
    /// Version (default: "latest")
    #[serde(default = "default_version")]
    version: String,
    /// Search query (matched against item names, case-insensitive)
    query: String,
    /// Maximum number of results (default: 20)
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_version() -> String {
    "latest".to_string()
}

fn default_limit() -> usize {
    20
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("search_docs")
        .description(
            "Search for items by name within a crate's documentation on docs.rs. \
             Returns matching functions, structs, traits, etc. with their paths \
             and brief descriptions. Case-insensitive substring match.",
        )
        .read_only()
        .idempotent()
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<SearchDocsInput>| async move {
                let krate = state
                    .docs_cache
                    .get_or_fetch(&state.docsrs_client, &input.name, &input.version)
                    .await
                    .tool_context("docs.rs fetch error")?;

                let query_lower = input.query.to_lowercase();
                let limit = input.limit.min(100);

                // Collect matches from local items only (crate_id == 0)
                let mut matches: Vec<_> = krate
                    .index
                    .iter()
                    .filter(|(_, item)| {
                        item.crate_id == 0
                            && item
                                .name
                                .as_ref()
                                .is_some_and(|n| n.to_lowercase().contains(&query_lower))
                    })
                    .collect();

                // Sort: exact match first, then prefix, then substring
                matches.sort_by(|(_, a), (_, b)| {
                    let a_name = a.name.as_deref().unwrap_or("").to_lowercase();
                    let b_name = b.name.as_deref().unwrap_or("").to_lowercase();
                    let a_exact = a_name == query_lower;
                    let b_exact = b_name == query_lower;
                    let a_prefix = a_name.starts_with(&query_lower);
                    let b_prefix = b_name.starts_with(&query_lower);

                    match (a_exact, b_exact) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => match (a_prefix, b_prefix) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => a_name.cmp(&b_name),
                        },
                    }
                });

                let total = matches.len();
                matches.truncate(limit);

                if matches.is_empty() {
                    return Ok(CallToolResult::text(format!(
                        "No items matching '{}' found in {} v{}.",
                        input.query, input.name, input.version
                    )));
                }

                let mut output = format!(
                    "Found {} items matching '{}' in {} v{} (showing {}):\n\n",
                    total,
                    input.query,
                    input.name,
                    input.version,
                    matches.len()
                );
                output.push_str(&format::format_search_results(&krate, &matches));

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
