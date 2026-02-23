//! Get keyword detail tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for getting a keyword
#[derive(Debug, Deserialize, JsonSchema)]
pub struct KeywordDetailInput {
    /// Keyword ID (e.g. "async", "cli", "parser", "serialization")
    id: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_keyword")
        .description(
            "Get details about a specific crates.io keyword, \
             including the number of crates using it.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<KeywordDetailInput>| async move {
                let kw = state
                    .client
                    .keyword(&input.id)
                    .await
                    .tool_context("Crates.io API error")?;

                let output = format!(
                    "# Keyword: {}\n\n\
                     **Crates:** {}\n\
                     **Browse:** https://crates.io/keywords/{}\n",
                    kw.keyword, kw.crates_cnt, kw.keyword
                );

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
