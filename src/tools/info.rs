//! Get crate info tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, format_number};

/// Input for getting crate info
#[derive(Debug, Deserialize, JsonSchema)]
pub struct InfoInput {
    /// Crate name
    name: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_crate_info")
        .description(
            "Get detailed information about a specific crate including description, \
             links, download stats, keywords, and categories.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<InfoInput>| async move {
                let response = state
                    .client
                    .get_crate(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                let c = &response.crate_data;

                let mut output = format!("# {}\n\n", c.name);

                if let Some(desc) = &c.description {
                    output.push_str(&format!("{}\n\n", desc.trim()));
                }

                output.push_str("## Versions\n\n");
                output.push_str(&format!("- Latest: **{}**\n", c.max_version));
                if let Some(stable) = &c.max_stable_version
                    && stable != &c.max_version
                {
                    output.push_str(&format!("- Latest Stable: **{}**\n", stable));
                }

                output.push_str("\n## Stats\n\n");
                output.push_str(&format!(
                    "- Total Downloads: {}\n",
                    format_number(c.downloads)
                ));
                if let Some(recent) = c.recent_downloads {
                    output.push_str(&format!("- Recent Downloads: {}\n", format_number(recent)));
                }
                output.push_str(&format!("- Created: {}\n", c.created_at.date_naive()));
                output.push_str(&format!("- Updated: {}\n", c.updated_at.date_naive()));

                output.push_str("\n## Links\n\n");
                if let Some(repo) = &c.repository {
                    output.push_str(&format!("- Repository: {}\n", repo));
                }
                if let Some(docs) = &c.documentation {
                    output.push_str(&format!("- Documentation: {}\n", docs));
                }
                if let Some(home) = &c.homepage {
                    output.push_str(&format!("- Homepage: {}\n", home));
                }

                if let Some(keywords) = &c.keywords
                    && !keywords.is_empty()
                {
                    output.push_str(&format!("\n## Keywords\n\n{}\n", keywords.join(", ")));
                }

                if let Some(categories) = &c.categories
                    && !categories.is_empty()
                {
                    output.push_str(&format!("\n## Categories\n\n{}\n", categories.join(", ")));
                }

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
