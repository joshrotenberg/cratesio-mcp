//! Get crates.io summary statistics

use std::sync::Arc;

use tower_mcp::{
    CallToolResult, NoParams, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, format_number};

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_summary")
        .description(
            "Get crates.io summary statistics including total crates, downloads, \
             new crates, most downloaded, and recently updated crates.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(_input): Json<NoParams>| async move {
                let summary = state
                    .client
                    .summary()
                    .await
                    .tool_context("Crates.io API error")?;

                let mut output = String::from("# Crates.io Summary\n\n");

                output.push_str("## Statistics\n\n");
                output.push_str(&format!(
                    "- Total Crates: {}\n",
                    format_number(summary.num_crates)
                ));
                output.push_str(&format!(
                    "- Total Downloads: {}\n",
                    format_number(summary.num_downloads)
                ));

                output.push_str("\n## New Crates\n\n");
                for c in summary.new_crates.iter().take(10) {
                    let desc = c
                        .description
                        .as_ref()
                        .map(|d| format!(" - {}", d.trim()))
                        .unwrap_or_default();
                    output.push_str(&format!("- **{}** v{}{}\n", c.name, c.max_version, desc));
                }

                output.push_str("\n## Most Downloaded\n\n");
                for c in summary.most_downloaded.iter().take(10) {
                    output.push_str(&format!(
                        "- **{}** ({} downloads)\n",
                        c.name,
                        format_number(c.downloads)
                    ));
                }

                output.push_str("\n## Most Recently Updated\n\n");
                for c in summary.just_updated.iter().take(10) {
                    output.push_str(&format!("- **{}** v{}\n", c.name, c.max_version));
                }

                output.push_str("\n## Popular Keywords\n\n");
                for kw in summary.popular_keywords.iter().take(10) {
                    output.push_str(&format!("- {} ({} crates)\n", kw.keyword, kw.crates_cnt));
                }

                output.push_str("\n## Popular Categories\n\n");
                for cat in summary.popular_categories.iter().take(10) {
                    output.push_str(&format!("- {} ({} crates)\n", cat.category, cat.crates_cnt));
                }

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
