//! Resource template for crate documentation
//!
//! Exposes crate docs as resources via URI template: crates://{name}/docs

use std::collections::HashMap;
use std::sync::Arc;

use tower_mcp::protocol::{ReadResourceResult, ResourceContent};
use tower_mcp::resource::{ResourceTemplate, ResourceTemplateBuilder};

use crate::docs::format;
use crate::state::AppState;

pub fn build(state: Arc<AppState>) -> ResourceTemplate {
    ResourceTemplateBuilder::new("crates://{name}/docs")
        .name("Crate Documentation")
        .description("Get the documentation structure for a crate from docs.rs")
        .mime_type("text/markdown")
        .handler(move |uri: String, vars: HashMap<String, String>| {
            let state = state.clone();
            async move {
                let name = vars.get("name").cloned().unwrap_or_default();

                let krate = state
                    .docs_cache
                    .get_or_fetch(&state.docsrs_client, &name, "latest")
                    .await
                    .map_err(|e| tower_mcp::Error::tool(format!("docs.rs fetch error: {}", e)))?;

                let output = format::format_module_listing(&krate, &krate.root);

                Ok(ReadResourceResult {
                    contents: vec![ResourceContent {
                        uri,
                        mime_type: Some("text/markdown".to_string()),
                        text: Some(output),
                        blob: None,
                        meta: None,
                    }],
                    meta: None,
                })
            }
        })
}
