//! Internal wire types for serde deserialization.
//!
//! These types match the raw JSON structure from the crates.io API
//! and are not exposed publicly.

use serde::{Deserialize, Serialize};

use super::types::{
    ApiToken, Category, Dependency, GitHubConfig, GitLabConfig, Keyword, Meta, OwnerInvitation,
    PublishWarnings, Team, User, Version,
};

// ── Wrapper types ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub(crate) struct OwnersResponse {
    pub users: Vec<User>,
}

#[derive(Deserialize)]
pub(crate) struct AuthorsResponse {
    pub meta: AuthorsMeta,
}

#[derive(Deserialize)]
pub(crate) struct AuthorsMeta {
    pub names: Vec<String>,
}

#[derive(Deserialize)]
pub(crate) struct UserResponse {
    pub user: User,
}

#[derive(Deserialize)]
pub(crate) struct VersionResponse {
    pub version: Version,
}

#[derive(Deserialize)]
pub(crate) struct CategoryResponse {
    pub category: Category,
}

#[derive(Deserialize)]
pub(crate) struct KeywordResponse {
    pub keyword: Keyword,
}

/// Raw reverse dependencies response before join.
#[derive(Deserialize)]
pub(crate) struct ReverseDependenciesRaw {
    pub dependencies: Vec<Dependency>,
    pub versions: Vec<RawVersion>,
    pub meta: Meta,
}

/// Minimal version info from the reverse dependencies endpoint.
#[derive(Deserialize)]
pub(crate) struct RawVersion {
    pub id: u64,
    #[serde(rename = "crate")]
    pub krate: String,
    pub num: String,
}

#[derive(Deserialize)]
pub(crate) struct DependenciesResponse {
    pub dependencies: Vec<Dependency>,
}

// ── Team wrapper ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub(crate) struct TeamResponse {
    pub team: Team,
}

// ── User stats wrapper ──────────────────────────────────────────────────────

#[derive(Deserialize)]
pub(crate) struct UserStatsResponse {
    pub total_downloads: u64,
}

// ── Category slugs wrapper ──────────────────────────────────────────────────

use super::types::CategorySlug;

#[derive(Deserialize)]
pub(crate) struct CategorySlugsResponse {
    pub category_slugs: Vec<CategorySlug>,
}

// ── Site metadata wrapper ───────────────────────────────────────────────────

#[derive(Deserialize)]
pub(crate) struct SiteMetadataResponse {
    pub deployed_sha: Option<String>,
    #[serde(default)]
    pub commit: Option<String>,
}

// ── Authenticated wire types ────────────────────────────────────────────────

/// Request body for add/remove owners.
#[derive(Serialize)]
pub(crate) struct OwnersRequest {
    pub users: Vec<String>,
}

/// Response for owner invitation list.
#[derive(Deserialize)]
pub(crate) struct OwnerInvitationsResponse {
    pub crate_owner_invitations: Vec<OwnerInvitation>,
}

/// Request body for handling an owner invitation.
#[derive(Serialize)]
pub(crate) struct HandleInvitationRequest {
    pub accepted: bool,
    pub crate_id: u64,
}

/// Response from accepting invitation by token.
#[derive(Deserialize)]
pub(crate) struct InvitationTokenResponse {
    pub crate_owner_invitation: InvitationTokenData,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct InvitationTokenData {
    pub accepted: bool,
    pub crate_id: u64,
}

/// Request body for updating crate settings.
#[derive(Serialize)]
pub(crate) struct UpdateCrateRequest {
    #[serde(rename = "crate")]
    pub crate_data: super::types::CrateSettings,
}

/// Request body for updating version settings.
#[derive(Serialize)]
pub(crate) struct UpdateVersionRequest {
    pub version: super::types::VersionSettings,
}

/// User update request.
#[derive(Serialize)]
pub(crate) struct UpdateUserRequest {
    pub user: UpdateUserData,
}

#[derive(Serialize)]
pub(crate) struct UpdateUserData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// My updates response.
#[derive(Deserialize)]
pub(crate) struct MyUpdatesResponse {
    pub versions: Vec<Version>,
    pub meta: MyUpdatesMeta,
}

#[derive(Deserialize)]
pub(crate) struct MyUpdatesMeta {
    #[serde(default)]
    pub more: bool,
}

// ── Token wire types ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub(crate) struct TokensResponse {
    pub api_tokens: Vec<ApiToken>,
}

#[derive(Deserialize)]
pub(crate) struct TokenResponse {
    pub api_token: ApiToken,
}

#[derive(Serialize)]
pub(crate) struct CreateTokenRequest {
    pub api_token: CreateTokenData,
}

#[derive(Serialize)]
pub(crate) struct CreateTokenData {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crate_scopes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_scopes: Option<Vec<String>>,
}

// ── Publish wire types ──────────────────────────────────────────────────────

#[derive(Deserialize)]
pub(crate) struct PublishResponse {
    pub warnings: PublishWarnings,
}

// ── Trusted publishing wire types ───────────────────────────────────────────

#[derive(Deserialize)]
pub(crate) struct GitHubConfigsResponse {
    pub github_configs: Vec<GitHubConfig>,
}

#[derive(Serialize)]
pub(crate) struct CreateGitHubConfigRequest {
    pub github_config: super::types::NewGitHubConfig,
}

#[derive(Deserialize)]
pub(crate) struct GitHubConfigResponse {
    pub github_config: GitHubConfig,
}

#[derive(Deserialize)]
pub(crate) struct GitLabConfigsResponse {
    pub gitlab_configs: Vec<GitLabConfig>,
}

#[derive(Serialize)]
pub(crate) struct CreateGitLabConfigRequest {
    pub gitlab_config: super::types::NewGitLabConfig,
}

#[derive(Deserialize)]
pub(crate) struct GitLabConfigResponse {
    pub gitlab_config: GitLabConfig,
}

/// Request body for OIDC token exchange.
#[derive(Serialize)]
pub(crate) struct OidcExchangeRequest {
    pub jwt: String,
}

/// Response from OIDC token exchange.
#[derive(Deserialize)]
pub(crate) struct OidcExchangeResponse {
    pub token: String,
}
