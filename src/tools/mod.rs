//! Tool definitions for crates.io queries
//!
//! Each tool corresponds to a crates.io API endpoint.

pub mod alternatives;
pub mod audit;
pub mod authors;
pub mod categories;
pub mod category;
pub mod changelog;
pub mod compare;
pub mod crate_docs;
pub mod dependencies;
pub mod dependency_tree;
pub mod doc_item;
pub mod downloads;
pub mod features;
pub mod health_check;
pub mod info;
pub mod keyword_detail;
pub mod keywords;
pub(crate) mod output;
pub mod owners;
pub mod readme;
pub mod release_timeline;
pub mod reverse_deps;
pub mod search;
pub mod search_docs;
pub mod summary;
pub mod user;
pub mod user_stats;
pub mod version_detail;
pub mod version_downloads;
pub mod versions;

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use schemars::JsonSchema;
    use serde::Serialize;
    use tower_mcp::Tool;

    use super::*;
    use crate::state::AppState;

    /// Number of tools this module defines. Pinned so that adding a tool
    /// without adding it to [`all_tools`] fails here rather than silently
    /// escaping the schema checks below.
    const TOOL_COUNT: usize = 29;

    /// An `AppState` that performs no I/O. These tests only read tool
    /// definitions; the clients are constructed but never called.
    fn state() -> Arc<AppState> {
        Arc::new(
            AppState::new(
                Duration::from_millis(0),
                Duration::from_secs(30),
                10,
                Duration::from_secs(3600),
            )
            .expect("AppState construction must not require network access"),
        )
    }

    /// Every tool defined in this module, built.
    ///
    /// Rust has no module reflection, so this list is maintained by hand. A new
    /// tool must be added here to be covered by the schema checks.
    fn all_tools() -> Vec<Tool> {
        let s = state();
        vec![
            alternatives::build(s.clone()),
            audit::build(s.clone()),
            authors::build(s.clone()),
            categories::build(s.clone()),
            category::build(s.clone()),
            changelog::build(s.clone()),
            compare::build(s.clone()),
            crate_docs::build(s.clone()),
            dependencies::build(s.clone()),
            dependency_tree::build(s.clone()),
            doc_item::build(s.clone()),
            downloads::build(s.clone()),
            features::build(s.clone()),
            health_check::build(s.clone()),
            info::build(s.clone()),
            keyword_detail::build(s.clone()),
            keywords::build(s.clone()),
            owners::build(s.clone()),
            readme::build(s.clone()),
            release_timeline::build(s.clone()),
            reverse_deps::build(s.clone()),
            search::build(s.clone()),
            search_docs::build(s.clone()),
            summary::build(s.clone()),
            user::build(s.clone()),
            user_stats::build(s.clone()),
            version_detail::build(s.clone()),
            version_downloads::build(s.clone()),
            versions::build(s),
        ]
    }

    #[tokio::test]
    async fn tool_count_is_pinned() {
        assert_eq!(
            all_tools().len(),
            TOOL_COUNT,
            "add the new tool to all_tools() so the schema checks cover it"
        );
    }

    /// MCP requires `Tool.outputSchema` to be an object schema. A strict client
    /// rejects the whole `tools/list` response when one tool violates it, so a
    /// single bad schema removes every tool from discovery.
    ///
    /// This is the regression guard for #164, where an internally tagged enum
    /// made schemars emit a root `oneOf` with no `"type"`.
    #[tokio::test]
    async fn all_output_schemas_are_object_rooted() {
        let tools = all_tools();
        let offenders: Vec<String> = tools
            .iter()
            .filter_map(|tool| {
                let schema = tool.output_schema.as_ref()?;
                let ty = schema.get("type").and_then(|t| t.as_str());
                if ty == Some("object") {
                    return None;
                }
                // Report the composition keyword when present, since that is
                // what a schema reaches for instead of a root type.
                let composed: Vec<&str> = ["oneOf", "anyOf", "allOf"]
                    .into_iter()
                    .filter(|k| schema.get(*k).is_some())
                    .collect();
                Some(format!(
                    "{}: type={:?} composition={:?}",
                    tool.name, ty, composed
                ))
            })
            .collect();

        assert!(
            offenders.is_empty(),
            "every tool's outputSchema must have a root \"type\": \"object\"; \
             these do not:\n  {}",
            offenders.join("\n  ")
        );
    }

    /// A tool that declares no output schema loses structured output silently.
    /// All tools currently declare one; this keeps it that way.
    #[tokio::test]
    async fn every_tool_declares_an_output_schema() {
        let tools = all_tools();
        let missing: Vec<&str> = tools
            .iter()
            .filter(|tool| tool.output_schema.is_none())
            .map(|tool| tool.name.as_str())
            .collect();

        assert!(
            missing.is_empty(),
            "these tools declare no outputSchema: {missing:?}"
        );
    }

    /// Proves the assertion above has teeth.
    ///
    /// This is the output shape `get_crate_changelog` had before #164: an
    /// internally tagged enum. schemars renders it as a root `oneOf` with no
    /// `"type"`, which is exactly what `all_output_schemas_are_object_rooted`
    /// rejects. If schemars ever started emitting a root type for tagged
    /// enums, this test would fail and the guard above would be measuring
    /// nothing.
    #[test]
    fn a_tagged_enum_still_produces_the_non_compliant_shape() {
        // Never constructed: it exists only so schemars can render its
        // schema, which is the shape under test.
        #[allow(dead_code)]
        #[derive(Serialize, JsonSchema)]
        #[serde(tag = "status", rename_all = "snake_case")]
        enum PreFixShape {
            Found { name: String },
            NotFound { name: String },
        }

        let schema = output::schema::<PreFixShape>();
        assert!(
            schema.get("type").is_none(),
            "expected no root type, got {:?}",
            schema.get("type")
        );
        assert!(
            schema.get("oneOf").is_some(),
            "expected a root oneOf, got keys {:?}",
            schema.as_object().map(|o| o.keys().collect::<Vec<_>>())
        );
    }

    /// The specific shape #164 settled on: a flat object with a `status`
    /// discriminant, rather than a tagged enum.
    #[tokio::test]
    async fn changelog_output_schema_is_an_object_with_a_status_discriminant() {
        let tool = changelog::build(state());
        let schema = tool
            .output_schema
            .expect("get_crate_changelog must declare an output schema");

        assert_eq!(schema.get("type").and_then(|t| t.as_str()), Some("object"));
        assert!(
            schema.get("oneOf").is_none(),
            "a root oneOf is what made this schema non-compliant"
        );

        let required: Vec<&str> = schema
            .get("required")
            .and_then(|r| r.as_array())
            .expect("required must be present")
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(required.contains(&"status"), "got {required:?}");
        assert!(required.contains(&"name"), "got {required:?}");

        let properties = schema
            .get("properties")
            .and_then(|p| p.as_object())
            .expect("properties must be present");
        for key in ["status", "name", "version", "filename", "content", "url"] {
            assert!(properties.contains_key(key), "missing property {key}");
        }
    }
}
