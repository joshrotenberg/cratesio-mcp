//! Get crate changelog tool

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::client::changelog::ChangelogResult;
use crate::state::AppState;

/// Input for fetching a crate's changelog
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ChangelogInput {
    /// Crate name
    name: String,
    /// Version to filter to (e.g. "1.2.3"). When provided, only the section
    /// for that version is returned from the changelog.
    #[serde(default)]
    version: Option<String>,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_crate_changelog")
        .title("Get Crate Changelog")
        .description(
            "Fetch changelog content for a crate from its GitHub repository. \
             Tries common filenames (CHANGELOG.md, CHANGES.md, HISTORY.md, RELEASES.md). \
             If a version is provided, returns only that version's section. \
             Only GitHub repositories are supported.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>, Json(input): Json<ChangelogInput>| async move {
                let result = state
                    .client
                    .fetch_changelog(&input.name)
                    .await
                    .tool_context("Failed to fetch changelog")?;

                match result {
                    ChangelogResult::NoRepository => Ok(CallToolResult::text(format!(
                        "No repository URL found for crate `{}`.",
                        input.name
                    ))),
                    ChangelogResult::NotGitHub { url } => Ok(CallToolResult::text(format!(
                        "Repository for `{}` is not on GitHub (`{}`). \
                         Only GitHub repositories are supported.",
                        input.name, url
                    ))),
                    ChangelogResult::NotFound => Ok(CallToolResult::text(format!(
                        "No changelog file found in the GitHub repository for `{}`. \
                         Tried: CHANGELOG.md, CHANGES.md, HISTORY.md, RELEASES.md.",
                        input.name
                    ))),
                    ChangelogResult::Found { filename, content } => {
                        let body = match &input.version {
                            Some(v) => extract_version_section(&content, v).unwrap_or_else(|| {
                                format!(
                                    "Version `{v}` section not found in changelog. \
                                         Returning full changelog.\n\n{}",
                                    content
                                )
                            }),
                            None => content,
                        };
                        Ok(CallToolResult::text(format!(
                            "# {} - {}\n\n{}",
                            input.name, filename, body
                        )))
                    }
                }
            },
        )
        .build()
}

/// Extract the section for a specific version from a changelog.
///
/// Looks for a heading line containing the version string (e.g. `## [1.2.3]`,
/// `## 1.2.3`, `## v1.2.3`) and returns from that heading up to (but not
/// including) the next heading of the same or higher level.
fn extract_version_section(content: &str, version: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();

    // Find the index of the heading that contains the version string.
    let start_idx = lines.iter().position(|line| {
        let stripped = line.trim_start_matches('#').trim();
        // Match "[version]", "version", or "vversion"
        stripped.contains(version) && (line.starts_with('#'))
    })?;

    let heading_level = lines[start_idx].chars().take_while(|c| *c == '#').count();

    // Find the next heading of the same or higher level (fewer #s).
    let end_idx = lines[start_idx + 1..]
        .iter()
        .position(|line| {
            let level = line.chars().take_while(|c| *c == '#').count();
            level > 0 && level <= heading_level
        })
        .map(|i| start_idx + 1 + i)
        .unwrap_or(lines.len());

    let section = lines[start_idx..end_idx].join("\n");
    if section.trim().is_empty() {
        None
    } else {
        Some(section)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::state::AppState;

    // Minimal crate info JSON with a GitHub repository URL.
    fn crate_json(repo: &str) -> String {
        format!(
            r#"{{
                "crate": {{
                    "name": "testcrate",
                    "updated_at": "2025-01-01T00:00:00Z",
                    "created_at": "2024-01-01T00:00:00Z",
                    "downloads": 1000,
                    "max_version": "1.0.0",
                    "max_stable_version": "1.0.0",
                    "repository": "{repo}"
                }},
                "versions": []
            }}"#
        )
    }

    const SAMPLE_CHANGELOG: &str = "\
# Changelog

## [2.0.0] - 2025-01-01
### Added
- New feature

## [1.0.0] - 2024-01-01
### Added
- Initial release
";

    #[tokio::test]
    async fn changelog_found() {
        let crates_server = MockServer::start().await;
        let github_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/testcrate"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                crate_json("https://github.com/example/testcrate"),
                "application/json",
            ))
            .mount(&crates_server)
            .await;

        // First filename tried is CHANGELOG.md
        Mock::given(method("GET"))
            .and(path("/example/testcrate/HEAD/CHANGELOG.md"))
            .respond_with(ResponseTemplate::new(200).set_body_string(SAMPLE_CHANGELOG))
            .mount(&github_server)
            .await;

        let state = Arc::new(
            AppState::with_changelog_urls(&crates_server.uri(), &github_server.uri()).unwrap(),
        );

        let input = ChangelogInput {
            name: "testcrate".to_string(),
            version: None,
        };

        let tool = build(state);
        let _ = tool; // tool is built; invoke handler logic directly via client

        // Test the client method directly
        let client = crate::client::CratesIoClient::with_base_url(
            "test",
            std::time::Duration::from_millis(0),
            &crates_server.uri(),
        )
        .unwrap()
        .with_github_raw_url(&github_server.uri());

        let result = client.fetch_changelog(&input.name).await.unwrap();
        assert!(matches!(result, ChangelogResult::Found { .. }));
    }

    #[tokio::test]
    async fn changelog_no_repository() {
        let crates_server = MockServer::start().await;
        let github_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/testcrate"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(crate_json(""), "application/json"),
            )
            .mount(&crates_server)
            .await;

        let client = crate::client::CratesIoClient::with_base_url(
            "test",
            std::time::Duration::from_millis(0),
            &crates_server.uri(),
        )
        .unwrap()
        .with_github_raw_url(&github_server.uri());

        let result = client.fetch_changelog("testcrate").await.unwrap();
        assert!(matches!(result, ChangelogResult::NoRepository));
    }

    #[tokio::test]
    async fn changelog_non_github_repo() {
        let crates_server = MockServer::start().await;
        let github_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/testcrate"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                crate_json("https://gitlab.com/example/testcrate"),
                "application/json",
            ))
            .mount(&crates_server)
            .await;

        let client = crate::client::CratesIoClient::with_base_url(
            "test",
            std::time::Duration::from_millis(0),
            &crates_server.uri(),
        )
        .unwrap()
        .with_github_raw_url(&github_server.uri());

        let result = client.fetch_changelog("testcrate").await.unwrap();
        assert!(matches!(result, ChangelogResult::NotGitHub { .. }));
    }

    #[tokio::test]
    async fn changelog_not_found_after_all_filenames() {
        let crates_server = MockServer::start().await;
        let github_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/testcrate"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                crate_json("https://github.com/example/testcrate"),
                "application/json",
            ))
            .mount(&crates_server)
            .await;

        // All filenames return 404
        for filename in &[
            "CHANGELOG.md",
            "CHANGES.md",
            "HISTORY.md",
            "RELEASES.md",
            "changelog.md",
            "changes.md",
            "history.md",
            "releases.md",
        ] {
            Mock::given(method("GET"))
                .and(path(format!("/example/testcrate/HEAD/{filename}")))
                .respond_with(ResponseTemplate::new(404))
                .mount(&github_server)
                .await;
        }

        let client = crate::client::CratesIoClient::with_base_url(
            "test",
            std::time::Duration::from_millis(0),
            &crates_server.uri(),
        )
        .unwrap()
        .with_github_raw_url(&github_server.uri());

        let result = client.fetch_changelog("testcrate").await.unwrap();
        assert!(matches!(result, ChangelogResult::NotFound));
    }

    #[test]
    fn extract_version_section_found() {
        let section = extract_version_section(SAMPLE_CHANGELOG, "1.0.0").unwrap();
        assert!(section.contains("## [1.0.0]"));
        assert!(section.contains("Initial release"));
        assert!(!section.contains("New feature"));
    }

    #[test]
    fn extract_version_section_first_entry() {
        let section = extract_version_section(SAMPLE_CHANGELOG, "2.0.0").unwrap();
        assert!(section.contains("## [2.0.0]"));
        assert!(section.contains("New feature"));
        assert!(!section.contains("Initial release"));
    }

    #[test]
    fn extract_version_section_not_found() {
        assert!(extract_version_section(SAMPLE_CHANGELOG, "99.0.0").is_none());
    }
}
