//! Search crates tool

use std::sync::Arc;

use crates_io_api::{CratesQuery, Sort};
use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, CrateSummary, format_number};

/// Input for searching crates
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchInput {
    /// Search query (crate name or keywords)
    query: String,
    /// Sort order: relevance, downloads, recent-downloads, recent-updates, new
    #[serde(default = "default_sort")]
    sort: String,
}

fn default_sort() -> String {
    "relevance".to_string()
}

fn parse_sort(s: &str) -> Sort {
    match s {
        "downloads" => Sort::Downloads,
        "recent-downloads" => Sort::RecentDownloads,
        "recent-updates" => Sort::RecentUpdates,
        "new" => Sort::NewlyAdded,
        _ => Sort::Relevance,
    }
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("search_crates")
        .description(
            "Search for Rust crates on crates.io. Returns crate names, descriptions, \
             download counts, and repository links.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<SearchInput>| async move {
                let sort = parse_sort(&input.sort);
                let query = CratesQuery::builder()
                    .search(&input.query)
                    .sort(sort)
                    .build();

                let response = state
                    .client
                    .crates(query)
                    .await
                    .tool_context("Crates.io API error")?;

                // Save search for resources
                let summaries: Vec<_> = response
                    .crates
                    .iter()
                    .map(|c| CrateSummary {
                        name: c.name.clone(),
                        description: c.description.clone(),
                        max_version: c.max_version.clone(),
                        downloads: c.downloads,
                    })
                    .collect();
                state.save_search(input.query.clone(), summaries).await;

                // Format results
                let mut output = format!(
                    "Found {} crates matching '{}' (showing {}):\n\n",
                    response.meta.total,
                    input.query,
                    response.crates.len()
                );

                for (i, c) in response.crates.iter().enumerate() {
                    output.push_str(&format!("{}. **{}** v{}\n", i + 1, c.name, c.max_version));
                    if let Some(desc) = &c.description {
                        output.push_str(&format!("   {}\n", desc.trim()));
                    }
                    output.push_str(&format!(
                        "   Downloads: {} | Recent: {}\n",
                        format_number(c.downloads),
                        c.recent_downloads.map(format_number).unwrap_or_default()
                    ));
                    if let Some(repo) = &c.repository {
                        output.push_str(&format!("   Repo: {}\n", repo));
                    }
                    output.push('\n');
                }

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
