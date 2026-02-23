//! Get user download stats tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::{AppState, format_number};

/// Input for getting user download statistics
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UserStatsInput {
    /// GitHub username
    username: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_user_stats")
        .description(
            "Get download statistics for a crates.io user. \
             Shows total downloads across all of the user's crates.",
        )
        .read_only()
        .idempotent()
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<UserStatsInput>| async move {
                let user = state
                    .client
                    .user(&input.username)
                    .await
                    .tool_context("Crates.io API error")?;

                let stats = state
                    .client
                    .user_stats(user.id)
                    .await
                    .tool_context("Crates.io API error")?;

                let mut output = format!("# User Stats: {}\n\n", user.login);

                if let Some(name) = &user.name {
                    output.push_str(&format!("**Name:** {}\n\n", name));
                }

                output.push_str(&format!(
                    "**Total downloads:** {}\n",
                    format_number(stats.total_downloads)
                ));

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
