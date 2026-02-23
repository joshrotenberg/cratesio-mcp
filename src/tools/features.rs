//! Get feature flags for a crate version

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for getting feature flags
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FeaturesInput {
    /// Crate name (e.g. "serde", "tokio")
    name: String,
    /// Version string (e.g. "1.0.0"). Defaults to latest version.
    version: Option<String>,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_crate_features")
        .description(
            "Get feature flags for a crate version. Shows all Cargo features \
             and their sub-feature/dependency activations. Useful for understanding \
             what optional functionality a crate provides.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<FeaturesInput>| async move {
                let version = match &input.version {
                    Some(v) => v.clone(),
                    None => {
                        let crate_resp = state
                            .client
                            .get_crate(&input.name)
                            .await
                            .tool_context("Crates.io API error")?;
                        crate_resp.crate_data.max_version
                    }
                };

                let features = state
                    .client
                    .crate_features(&input.name, &version)
                    .await
                    .tool_context("Crates.io API error")?;

                let mut output = format!("# {} v{} - Feature Flags\n\n", input.name, version);

                if features.is_empty() {
                    output.push_str("No feature flags defined.\n");
                    return Ok(CallToolResult::text(output));
                }

                // Show default features first
                if let Some(defaults) = features.get("default") {
                    output.push_str("## Default Features\n\n");
                    if defaults.is_empty() {
                        output.push_str("_(none)_\n\n");
                    } else {
                        for f in defaults {
                            output.push_str(&format!("- `{f}`\n"));
                        }
                        output.push('\n');
                    }
                }

                // Collect and sort remaining features
                let mut others: Vec<_> = features
                    .iter()
                    .filter(|(k, _)| k.as_str() != "default")
                    .collect();
                others.sort_by_key(|(k, _)| k.as_str());

                if !others.is_empty() {
                    output.push_str("## Features\n\n");
                    for (name, deps) in &others {
                        if deps.is_empty() {
                            output.push_str(&format!("- `{name}`\n"));
                        } else {
                            let dep_list = deps
                                .iter()
                                .map(|d| format!("`{d}`"))
                                .collect::<Vec<_>>()
                                .join(", ");
                            output.push_str(&format!("- `{name}` -> {dep_list}\n"));
                        }
                    }
                    output.push('\n');
                }

                output.push_str(&format!("**Total: {} feature flags**\n", features.len()));

                Ok(CallToolResult::text(output))
            },
        )
        .build()
}
