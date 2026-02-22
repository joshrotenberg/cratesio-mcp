//! Recent searches resource

use std::sync::Arc;

use tower_mcp::{ReadResourceResult, Resource, ResourceBuilder, ResourceContent};

use crate::state::AppState;

pub fn build(state: Arc<AppState>) -> Resource {
    ResourceBuilder::new("crates://recent-searches")
        .name("Recent Searches")
        .description("JSON list of recent crate searches performed in this session")
        .mime_type("application/json")
        .handler(move || {
            let state = state.clone();
            async move {
                let searches = state.recent_searches.read().await;
                let json =
                    serde_json::to_string_pretty(&*searches).unwrap_or_else(|_| "[]".to_string());

                Ok(ReadResourceResult {
                    contents: vec![ResourceContent {
                        uri: "crates://recent-searches".to_string(),
                        mime_type: Some("application/json".to_string()),
                        text: Some(json),
                        blob: None,
                        meta: None,
                    }],
                    meta: None,
                })
            }
        })
        .build()
}
