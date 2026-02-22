//! Resource template for crate information
//!
//! Exposes crate info as resources via URI template: crates://{name}/info

use std::collections::HashMap;
use std::sync::Arc;

use tower_mcp::protocol::{ReadResourceResult, ResourceContent};
use tower_mcp::resource::{ResourceTemplate, ResourceTemplateBuilder};

use crate::state::{AppState, format_number};

pub fn build(state: Arc<AppState>) -> ResourceTemplate {
    ResourceTemplateBuilder::new("crates://{name}/info")
        .name("Crate Information")
        .description("Get detailed information about a crate by name")
        .mime_type("text/markdown")
        .handler(move |uri: String, vars: HashMap<String, String>| {
            let state = state.clone();
            async move {
                let name = vars.get("name").cloned().unwrap_or_default();

                let response =
                    state.client.get_crate(&name).await.map_err(|e| {
                        tower_mcp::Error::tool(format!("Crates.io API error: {}", e))
                    })?;

                let c = &response.crate_data;

                let mut content = format!("# {}\n\n", c.name);

                if let Some(desc) = &c.description {
                    content.push_str(&format!("{}\n\n", desc.trim()));
                }

                content.push_str("## Stats\n\n");
                content.push_str(&format!("- **Version:** {}\n", c.max_version));
                content.push_str(&format!(
                    "- **Downloads:** {}\n",
                    format_number(c.downloads)
                ));
                content.push_str(&format!("- **Created:** {}\n", c.created_at.date_naive()));
                content.push_str(&format!("- **Updated:** {}\n", c.updated_at.date_naive()));

                if let Some(repo) = &c.repository {
                    content.push_str(&format!("\n**Repository:** {}\n", repo));
                }

                if let Some(docs) = &c.documentation {
                    content.push_str(&format!("**Documentation:** {}\n", docs));
                }

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
