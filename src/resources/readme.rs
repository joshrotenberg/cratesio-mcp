//! Resource template for crate README content
//!
//! Exposes crate READMEs as resources via URI template: crates://{name}/readme

use std::collections::HashMap;
use std::sync::Arc;

use tower_mcp::protocol::{ReadResourceResult, ResourceContent};
use tower_mcp::resource::{ResourceTemplate, ResourceTemplateBuilder};

use crate::state::AppState;

pub fn build(state: Arc<AppState>) -> ResourceTemplate {
    ResourceTemplateBuilder::new("crates://{name}/readme")
        .name("Crate README")
        .description("Get the README content for a crate")
        .mime_type("text/markdown")
        .handler(move |uri: String, vars: HashMap<String, String>| {
            let state = state.clone();
            async move {
                let name = vars.get("name").cloned().unwrap_or_default();

                let response =
                    state.client.get_crate(&name).await.map_err(|e| {
                        tower_mcp::Error::tool(format!("Crates.io API error: {}", e))
                    })?;

                let version = response.crate_data.max_version.clone();

                let readme = state
                    .client
                    .crate_readme(&name, &version)
                    .await
                    .map_err(|e| tower_mcp::Error::tool(format!("Crates.io API error: {}", e)))?;

                let content = if readme.trim().is_empty() {
                    format!("No README found for {} v{}", name, version)
                } else {
                    format!("# {} v{} - README\n\n{}", name, version, readme)
                };

                Ok(ReadResourceResult {
                    contents: vec![ResourceContent {
                        uri,
                        mime_type: Some("text/markdown".to_string()),
                        text: Some(content),
                        blob: None,
                        meta: None,
                    }],
                    meta: None,
                })
            }
        })
}
