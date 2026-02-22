//! Get owners tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for getting owners
#[derive(Debug, Deserialize, JsonSchema)]
pub struct OwnersInput {
    /// Crate name
    name: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_owners")
        .description(
            "Get the owners/maintainers of a crate. Shows GitHub usernames \
             and team memberships.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<OwnersInput>| async move {
                let owners = state
                    .client
                    .crate_owners(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                let mut output = format!("# {} - Owners\n\n", input.name);

                let (users, teams): (Vec<_>, Vec<_>) = owners
                    .iter()
                    .partition(|o| o.kind.as_deref() == Some("user"));

                if !users.is_empty() {
                    output.push_str("## Users\n\n");
                    for owner in &users {
                        output.push_str(&format!("- **{}**", owner.login));
                        if let Some(name) = &owner.name {
                            output.push_str(&format!(" ({})", name));
                        }
                        output.push('\n');
                    }
                }

                if !teams.is_empty() {
                    output.push_str("\n## Teams\n\n");
                    for owner in &teams {
                        output.push_str(&format!("- **{}**\n", owner.login));
                    }
                }

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
