//! Get reverse dependencies tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::protocol::{LogLevel, LoggingMessageParams};
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Context, Json, State},
};

use crate::state::AppState;

/// Input for getting reverse dependencies
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReverseDepsInput {
    /// Crate name
    name: String,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_reverse_dependencies")
        .description(
            "Get crates that depend on the specified crate (reverse dependencies). \
             Useful for understanding a crate's ecosystem impact.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>,
             ctx: Context,
             Json(input): Json<ReverseDepsInput>| async move {
                // Log the request
                ctx.send_log(LoggingMessageParams {
                    level: LogLevel::Info,
                    logger: Some("cratesio-mcp".to_string()),
                    data: serde_json::json!({
                        "action": "fetch_reverse_deps",
                        "crate": input.name
                    }),
                    meta: None,
                });

                // Send initial progress
                ctx.report_progress(0.1, Some(1.0), Some("Fetching reverse dependencies..."))
                    .await;

                let response = state
                    .client
                    .crate_reverse_dependencies(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                // Update progress
                ctx.report_progress(0.8, Some(1.0), Some("Processing results..."))
                    .await;

                // Log the result count
                ctx.send_log(LoggingMessageParams {
                    level: LogLevel::Info,
                    logger: Some("cratesio-mcp".to_string()),
                    data: serde_json::json!({
                        "action": "fetch_reverse_deps_complete",
                        "crate": input.name,
                        "count": response.meta.total
                    }),
                    meta: None,
                });

                let mut output = format!(
                    "# {} - Reverse Dependencies\n\n\
                     {} crates depend on this crate (showing first {}):\n\n",
                    input.name,
                    response.meta.total,
                    response.dependencies.len()
                );

                for dep in &response.dependencies {
                    output.push_str(&format!(
                        "- **{}** v{} ({})\n",
                        dep.crate_version.crate_name, dep.crate_version.num, dep.dependency.req
                    ));
                }

                // Complete progress
                ctx.report_progress(1.0, Some(1.0), Some("Done")).await;

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
