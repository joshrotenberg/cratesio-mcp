//! Get category detail tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for getting a category
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CategoryInput {
    /// Category slug (e.g. "command-line-utilities", "web-programming", "cryptography")
    slug: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_category")
        .description(
            "Get details about a specific crates.io category by slug, \
             including its description and crate count.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<CategoryInput>| async move {
                let cat = state
                    .client
                    .category(&input.slug)
                    .await
                    .tool_context("Crates.io API error")?;

                let mut output = format!("# Category: {}\n\n", cat.category);

                if let Some(desc) = &cat.description {
                    output.push_str(&format!("{}\n\n", desc));
                }

                output.push_str(&format!("**Crates:** {}\n", cat.crates_cnt));

                if let Some(slug) = &cat.slug {
                    output.push_str(&format!(
                        "**Browse:** https://crates.io/categories/{}\n",
                        slug
                    ));
                }

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
