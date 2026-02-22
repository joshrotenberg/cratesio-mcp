//! Get dependencies tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for getting dependencies
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DependenciesInput {
    /// Crate name
    name: String,
    /// Version (default: latest)
    version: Option<String>,
    /// Include dev dependencies
    #[serde(default)]
    include_dev: bool,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_dependencies")
        .description(
            "Get dependencies for a crate version. Shows required and optional deps, \
             version requirements, and whether they're build or dev dependencies.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<DependenciesInput>| async move {
                // Get crate info first to find version
                let crate_response = state
                    .client
                    .get_crate(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                let version = input
                    .version
                    .as_deref()
                    .unwrap_or(&crate_response.crate_data.max_version);

                let deps = state
                    .client
                    .crate_dependencies(&input.name, version)
                    .await
                    .tool_context("Crates.io API error")?;

                let (normal, dev, build): (Vec<_>, Vec<_>, Vec<_>) =
                    deps.iter().fold((vec![], vec![], vec![]), |mut acc, d| {
                        match d.kind.as_str() {
                            "dev" => acc.1.push(d),
                            "build" => acc.2.push(d),
                            _ => acc.0.push(d),
                        }
                        acc
                    });

                let mut output = format!("# {} v{} - Dependencies\n\n", input.name, version);

                if !normal.is_empty() {
                    output.push_str("## Dependencies\n\n");
                    for d in &normal {
                        let optional = if d.optional { " (optional)" } else { "" };
                        output.push_str(&format!("- **{}** {}{}\n", d.crate_id, d.req, optional));
                    }
                    output.push('\n');
                }

                if !build.is_empty() {
                    output.push_str("## Build Dependencies\n\n");
                    for d in &build {
                        let optional = if d.optional { " (optional)" } else { "" };
                        output.push_str(&format!("- **{}** {}{}\n", d.crate_id, d.req, optional));
                    }
                    output.push('\n');
                }

                if input.include_dev && !dev.is_empty() {
                    output.push_str("## Dev Dependencies\n\n");
                    for d in &dev {
                        let optional = if d.optional { " (optional)" } else { "" };
                        output.push_str(&format!("- **{}** {}{}\n", d.crate_id, d.req, optional));
                    }
                    output.push('\n');
                }

                let total =
                    normal.len() + build.len() + if input.include_dev { dev.len() } else { 0 };
                output.push_str(&format!("**Total: {} dependencies**\n", total));

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
