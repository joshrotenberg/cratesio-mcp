//! Helpers for additive MCP structured tool output.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use tower_mcp::{CallToolResult, Result};

use crate::client::{User, UserStats};

/// A named, optionally versioned collection returned by a registry tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CollectionOutput<T> {
    pub name: String,
    pub version: Option<String>,
    pub total: u64,
    pub items: Vec<T>,
}

/// Markdown source fetched for a crate or one of its documentation items.
#[derive(Debug, Serialize, JsonSchema)]
pub struct DocumentOutput {
    pub name: String,
    pub version: String,
    pub path: Option<String>,
    pub content: Option<String>,
}

/// Cargo feature flags for a specific crate version.
#[derive(Debug, Serialize, JsonSchema)]
pub struct FeaturesOutput {
    pub name: String,
    pub version: String,
    pub features: HashMap<String, Vec<String>>,
}

/// A crates.io user and their aggregate download statistics.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UserStatsOutput {
    pub user: User,
    pub stats: UserStats,
}

/// A single documentation-search match.
#[derive(Debug, Serialize, JsonSchema)]
pub struct DocSearchMatch {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub summary: String,
}

/// Search results from a crate's rustdoc index.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchDocsOutput {
    pub name: String,
    pub version: String,
    pub query: String,
    pub total: u64,
    pub matches: Vec<DocSearchMatch>,
}

/// Generate the JSON Schema advertised by a tool for its structured result.
pub fn schema<T: JsonSchema>() -> Value {
    serde_json::to_value(schemars::schema_for!(T))
        .expect("schemars-generated output schemas must serialize")
}

/// Preserve a tool's human-readable Markdown while attaching typed JSON data.
pub fn structured<T: Serialize>(markdown: String, value: &T) -> Result<CallToolResult> {
    let mut result = CallToolResult::from_serialize(value)?;
    result.content = CallToolResult::text(markdown).content;
    Ok(result)
}
