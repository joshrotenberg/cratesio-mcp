//! Trusted publishing configuration endpoints.

use super::CratesIoClient;
use super::error::Error;
use super::types::{GitHubConfig, GitLabConfig, NewGitHubConfig, NewGitLabConfig};
use super::wire::{
    CreateGitHubConfigRequest, CreateGitLabConfigRequest, GitHubConfigResponse,
    GitHubConfigsResponse, GitLabConfigResponse, GitLabConfigsResponse, OidcExchangeRequest,
    OidcExchangeResponse,
};

impl CratesIoClient {
    // ── GitHub configs ──────────────────────────────────────────────────

    /// List all GitHub trusted publishing configs for the authenticated user.
    ///
    /// Requires authentication.
    pub async fn list_github_configs(&self) -> Result<Vec<GitHubConfig>, Error> {
        let resp: GitHubConfigsResponse = self.get_json_auth("/trustpub/github_configs").await?;
        Ok(resp.github_configs)
    }

    /// Create a new GitHub trusted publishing config.
    ///
    /// Requires authentication.
    pub async fn create_github_config(
        &self,
        config: NewGitHubConfig,
    ) -> Result<GitHubConfig, Error> {
        let body = CreateGitHubConfigRequest {
            github_config: config,
        };
        let resp: GitHubConfigResponse = self.post_json("/trustpub/github_configs", &body).await?;
        Ok(resp.github_config)
    }

    /// Delete a GitHub trusted publishing config.
    ///
    /// Requires authentication.
    pub async fn delete_github_config(&self, id: u64) -> Result<(), Error> {
        self.delete_ok(&format!("/trustpub/github_configs/{id}"))
            .await
    }

    // ── GitLab configs ──────────────────────────────────────────────────

    /// List all GitLab trusted publishing configs for the authenticated user.
    ///
    /// Requires authentication.
    pub async fn list_gitlab_configs(&self) -> Result<Vec<GitLabConfig>, Error> {
        let resp: GitLabConfigsResponse = self.get_json_auth("/trustpub/gitlab_configs").await?;
        Ok(resp.gitlab_configs)
    }

    /// Create a new GitLab trusted publishing config.
    ///
    /// Requires authentication.
    pub async fn create_gitlab_config(
        &self,
        config: NewGitLabConfig,
    ) -> Result<GitLabConfig, Error> {
        let body = CreateGitLabConfigRequest {
            gitlab_config: config,
        };
        let resp: GitLabConfigResponse = self.post_json("/trustpub/gitlab_configs", &body).await?;
        Ok(resp.gitlab_config)
    }

    /// Delete a GitLab trusted publishing config.
    ///
    /// Requires authentication.
    pub async fn delete_gitlab_config(&self, id: u64) -> Result<(), Error> {
        self.delete_ok(&format!("/trustpub/gitlab_configs/{id}"))
            .await
    }

    // ── OIDC token exchange ─────────────────────────────────────────────

    /// Exchange a CI OIDC JWT for a crates.io publish token.
    ///
    /// This endpoint does not require a crates.io API token; the OIDC JWT
    /// itself provides authentication.
    pub async fn exchange_oidc_token(&self, jwt: &str) -> Result<String, Error> {
        let body = OidcExchangeRequest {
            jwt: jwt.to_string(),
        };
        let resp: OidcExchangeResponse = self
            .post_json_unauth("/trustpub/tokens/exchange", &body)
            .await?;
        Ok(resp.token)
    }

    /// Revoke a trusted publishing token.
    ///
    /// Requires authentication.
    pub async fn revoke_trusted_token(&self, id: u64) -> Result<(), Error> {
        self.delete_ok(&format!("/trustpub/tokens/{id}")).await
    }
}
