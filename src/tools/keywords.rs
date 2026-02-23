//! Get keywords tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for listing keywords
#[derive(Debug, Deserialize, JsonSchema)]
pub struct KeywordsInput {
    /// Page number (default: 1)
    #[serde(default = "default_page")]
    page: u64,
    /// Results per page (default: 20, max: 100)
    #[serde(default = "default_per_page")]
    per_page: u64,
}

fn default_page() -> u64 {
    1
}

fn default_per_page() -> u64 {
    20
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_keywords")
        .description(
            "List crates.io keywords with the number of crates using each. \
             Useful for discovering crates by tag (e.g., async, cli, parser, serialization).",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<KeywordsInput>| async move {
                let response = state
                    .client
                    .keywords(Some(input.page), Some(input.per_page))
                    .await
                    .tool_context("Crates.io API error")?;

                let mut output = format!(
                    "# Crates.io Keywords (page {}, {} total)\n\n",
                    input.page, response.meta.total
                );

                for kw in &response.keywords {
                    output.push_str(&format!(
                        "- **{}** ({} crates)\n",
                        kw.keyword, kw.crates_cnt
                    ));
                }

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
