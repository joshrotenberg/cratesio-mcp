//! Changelog fetching from GitHub repositories.

use super::{CratesIoClient, Error};

/// The result of a changelog fetch attempt.
pub enum ChangelogResult {
    /// Changelog content found. `filename` is the matched path relative to the
    /// repository root, so a workspace crate reports `tokio/CHANGELOG.md`
    /// rather than a bare `CHANGELOG.md`.
    Found { filename: String, content: String },
    /// The crate has no repository URL set.
    NoRepository,
    /// The repository is not hosted on GitHub.
    NotGitHub { url: String },
    /// Repository is on GitHub but no changelog file was found.
    NotFound,
}

/// Common changelog filenames to try, in order of preference.
const CHANGELOG_FILENAMES: &[&str] = &[
    "CHANGELOG.md",
    "CHANGES.md",
    "HISTORY.md",
    "RELEASES.md",
    "changelog.md",
    "changes.md",
    "history.md",
    "releases.md",
];

impl CratesIoClient {
    /// Fetch changelog content for a crate from its GitHub repository.
    ///
    /// Looks up the crate's repository URL, and if it points to GitHub, tries
    /// common changelog filenames via the raw.githubusercontent.com CDN, first
    /// at the repository root and then under the crate's own directory, which
    /// is where most Cargo workspaces keep them.
    /// Returns a [`ChangelogResult`] describing what was found (or why nothing
    /// could be fetched).
    pub async fn fetch_changelog(&self, name: &str) -> Result<ChangelogResult, Error> {
        let crate_resp = self.get_crate(name).await?;
        let repo_url = match crate_resp.crate_data.repository {
            Some(url) if !url.is_empty() => url,
            _ => return Ok(ChangelogResult::NoRepository),
        };

        // Parse owner/repo from a GitHub URL.
        // Accepts https://github.com/owner/repo (with or without trailing slash / .git)
        let (owner, repo) = match parse_github_repo(&repo_url) {
            Some(pair) => pair,
            None => return Ok(ChangelogResult::NotGitHub { url: repo_url }),
        };

        // Candidate directories, in order. Root first, so a crate whose
        // changelog is already at the root costs no extra requests.
        //
        // The crate-named directory is the only fallback with evidence behind
        // it. A survey of 20 popular crates found 11 changelogs at the root, 5
        // under `<crate-name>/` (tokio, tracing, tower, tokio-util,
        // tracing-subscriber) and none under `crates/<crate-name>/`; the four
        // with no changelog anywhere were checked against all filenames under
        // all three prefixes. See #175.
        let prefixes = [String::new(), format!("{name}/")];

        for prefix in &prefixes {
            for filename in CHANGELOG_FILENAMES {
                let path = format!("{prefix}{filename}");
                let url = format!(
                    "{}/{}/{}/HEAD/{}",
                    self.github_raw_base_url, owner, repo, path
                );
                let resp = self.http.get(&url).send().await?;
                if resp.status().is_success() {
                    let content = resp.text().await?;
                    return Ok(ChangelogResult::Found {
                        // The matched path, so the caller can tell
                        // `CHANGELOG.md` from `tokio/CHANGELOG.md`.
                        filename: path,
                        content,
                    });
                }
            }
        }

        Ok(ChangelogResult::NotFound)
    }
}

/// Extract `(owner, repo)` from a GitHub URL, or return `None` if it is not
/// a recognisable GitHub URL.
fn parse_github_repo(url: &str) -> Option<(String, String)> {
    // Strip common prefixes
    let rest = url
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))?;

    let mut parts = rest.splitn(3, '/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();

    if owner.is_empty() || repo.is_empty() {
        return None;
    }

    Some((owner, repo))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::CratesIoClient;
    use std::time::Duration;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn crate_json(name: &str, repository: Option<&str>) -> serde_json::Value {
        let mut c = serde_json::json!({
            "name": name,
            "max_version": "1.0.0",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
        });
        if let Some(r) = repository {
            c["repository"] = serde_json::json!(r);
        }
        serde_json::json!({ "crate": c, "versions": [] })
    }

    #[tokio::test]
    async fn fetch_changelog_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/mycrate"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(crate_json("mycrate", Some("https://github.com/owner/repo"))),
            )
            .mount(&server)
            .await;
        // First filename tried (CHANGELOG.md) resolves on the raw host.
        Mock::given(method("GET"))
            .and(path("/owner/repo/HEAD/CHANGELOG.md"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string("# Changelog\n\n## 1.0.0\n- initial"),
            )
            .mount(&server)
            .await;

        let client = CratesIoClient::with_base_url(
            "test",
            Duration::from_millis(0),
            Duration::from_secs(5),
            &server.uri(),
        )
        .unwrap()
        .with_github_raw_url(&server.uri());
        match client.fetch_changelog("mycrate").await.unwrap() {
            ChangelogResult::Found { filename, content } => {
                assert_eq!(filename, "CHANGELOG.md");
                assert!(content.contains("# Changelog"));
            }
            _ => panic!("expected ChangelogResult::Found"),
        }
    }

    /// The #175 case: a workspace crate whose changelog lives under its own
    /// directory. Unmounted paths 404 in wiremock, so the root candidates all
    /// miss and the fallback has to find it.
    #[tokio::test]
    async fn fetch_changelog_found_in_crate_subdirectory() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/mycrate"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(crate_json("mycrate", Some("https://github.com/owner/repo"))),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/owner/repo/HEAD/mycrate/CHANGELOG.md"))
            .respond_with(ResponseTemplate::new(200).set_body_string("# Sub\n\n## 2.0.0\n- sub"))
            .mount(&server)
            .await;

        let client = CratesIoClient::with_base_url(
            "test",
            Duration::from_millis(0),
            Duration::from_secs(5),
            &server.uri(),
        )
        .unwrap()
        .with_github_raw_url(&server.uri());
        match client.fetch_changelog("mycrate").await.unwrap() {
            ChangelogResult::Found { filename, content } => {
                // The matched path, not the bare filename: that is what tells
                // the caller where it came from.
                assert_eq!(filename, "mycrate/CHANGELOG.md");
                assert!(content.contains("# Sub"));
            }
            _ => panic!("expected ChangelogResult::Found"),
        }
    }

    /// Root must win when both exist, so crates that already worked keep their
    /// current answer and pay nothing extra.
    #[tokio::test]
    async fn fetch_changelog_prefers_root_over_subdirectory() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/mycrate"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(crate_json("mycrate", Some("https://github.com/owner/repo"))),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/owner/repo/HEAD/CHANGELOG.md"))
            .respond_with(ResponseTemplate::new(200).set_body_string("# Root"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/owner/repo/HEAD/mycrate/CHANGELOG.md"))
            .respond_with(ResponseTemplate::new(200).set_body_string("# Sub"))
            .mount(&server)
            .await;

        let client = CratesIoClient::with_base_url(
            "test",
            Duration::from_millis(0),
            Duration::from_secs(5),
            &server.uri(),
        )
        .unwrap()
        .with_github_raw_url(&server.uri());
        match client.fetch_changelog("mycrate").await.unwrap() {
            ChangelogResult::Found { filename, content } => {
                assert_eq!(filename, "CHANGELOG.md");
                assert!(content.contains("# Root"));
            }
            _ => panic!("expected Found"),
        }
    }

    /// `serde` in miniature: a GitHub repository with no changelog file under
    /// either prefix must still report NotFound rather than inventing one.
    #[tokio::test]
    async fn fetch_changelog_not_found_under_either_prefix() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/mycrate"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(crate_json("mycrate", Some("https://github.com/owner/repo"))),
            )
            .mount(&server)
            .await;

        let client = CratesIoClient::with_base_url(
            "test",
            Duration::from_millis(0),
            Duration::from_secs(5),
            &server.uri(),
        )
        .unwrap()
        .with_github_raw_url(&server.uri());
        assert!(matches!(
            client.fetch_changelog("mycrate").await.unwrap(),
            ChangelogResult::NotFound
        ));
    }

    #[tokio::test]
    async fn fetch_changelog_no_repository() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/norepo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(crate_json("norepo", None)))
            .mount(&server)
            .await;

        let client = CratesIoClient::with_base_url(
            "test",
            Duration::from_millis(0),
            Duration::from_secs(5),
            &server.uri(),
        )
        .unwrap();
        assert!(matches!(
            client.fetch_changelog("norepo").await.unwrap(),
            ChangelogResult::NoRepository
        ));
    }

    #[test]
    fn parse_github_repo_standard() {
        let (owner, repo) = parse_github_repo("https://github.com/serde-rs/serde").unwrap();
        assert_eq!(owner, "serde-rs");
        assert_eq!(repo, "serde");
    }

    #[test]
    fn parse_github_repo_trailing_slash() {
        let (owner, repo) = parse_github_repo("https://github.com/tokio-rs/tokio/").unwrap();
        assert_eq!(owner, "tokio-rs");
        assert_eq!(repo, "tokio");
    }

    #[test]
    fn parse_github_repo_git_suffix() {
        let (owner, repo) = parse_github_repo("https://github.com/dtolnay/anyhow.git").unwrap();
        assert_eq!(owner, "dtolnay");
        assert_eq!(repo, "anyhow");
    }

    #[test]
    fn parse_github_repo_with_subpath() {
        // URLs with extra path components (monorepos) — still extract owner/repo
        let (owner, repo) =
            parse_github_repo("https://github.com/rust-lang/rust/tree/master/library/std").unwrap();
        assert_eq!(owner, "rust-lang");
        assert_eq!(repo, "rust");
    }

    #[test]
    fn parse_github_repo_non_github() {
        assert!(parse_github_repo("https://gitlab.com/owner/repo").is_none());
        assert!(parse_github_repo("https://bitbucket.org/owner/repo").is_none());
        assert!(parse_github_repo("").is_none());
    }

    #[test]
    fn parse_github_repo_incomplete() {
        assert!(parse_github_repo("https://github.com/").is_none());
        assert!(parse_github_repo("https://github.com/owner").is_none());
    }
}
