//! Get categories tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for listing categories
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CategoriesInput {
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
    ToolBuilder::new("get_categories")
        .description(
            "List crates.io categories with the number of crates in each. \
             Useful for discovering crates by domain (e.g., web-programming, \
             cryptography, database-implementations).",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<CategoriesInput>| async move {
                let response = state
                    .client
                    .categories(Some(input.page), Some(input.per_page))
                    .await
                    .tool_context("Crates.io API error")?;

                let mut output = format!(
                    "# Crates.io Categories (page {}, {} total)\n\n",
                    input.page, response.meta.total
                );

                for cat in &response.categories {
                    output.push_str(&format!(
                        "- **{}** ({} crates)\n",
                        cat.category, cat.crates_cnt
                    ));
                }

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
