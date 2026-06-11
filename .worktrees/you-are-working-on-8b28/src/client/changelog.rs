//! Changelog fetching from GitHub repositories.

use super::{CratesIoClient, Error};

/// The result of a changelog fetch attempt.
pub enum ChangelogResult {
    /// Changelog content found.
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
    /// common changelog filenames via the raw.githubusercontent.com CDN.
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

        // Try each common filename.
        for filename in CHANGELOG_FILENAMES {
            let url = format!(
                "{}/{}/{}/HEAD/{}",
                self.github_raw_base_url, owner, repo, filename
            );
            let resp = self.http.get(&url).send().await?;
            if resp.status().is_success() {
                let content = resp.text().await?;
                return Ok(ChangelogResult::Found {
                    filename: filename.to_string(),
                    content,
                });
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
