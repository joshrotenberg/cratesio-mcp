//! Public data types for the crates.io API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Core types ──────────────────────────────────────────────────────────────

/// Crate metadata from the crates.io API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Crate {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub max_version: String,
    #[serde(default)]
    pub max_stable_version: Option<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub recent_downloads: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub documentation: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    #[serde(default)]
    pub categories: Option<Vec<String>>,
}

/// Version metadata from the crates.io API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub num: String,
    #[serde(default)]
    pub yanked: bool,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub rust_version: Option<String>,
}

/// Per-version download data point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionDownloads {
    pub version: u64,
    pub downloads: u64,
    #[serde(default)]
    pub date: Option<String>,
}

/// User or team on crates.io.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

/// Dependency of a crate version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub crate_id: String,
    pub req: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub version_id: u64,
}

/// A keyword from crates.io.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyword {
    pub keyword: String,
    #[serde(default)]
    pub crates_cnt: u64,
}

/// A category from crates.io.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub category: String,
    #[serde(default)]
    pub crates_cnt: u64,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Pagination metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub total: u64,
}

/// Authors listed in a crate version's Cargo.toml.
#[derive(Debug, Clone)]
pub struct Authors {
    pub names: Vec<String>,
}

/// A reverse dependency entry (a crate that depends on the queried crate).
#[derive(Debug, Clone)]
pub struct ReverseDependency {
    pub crate_version: CrateVersion,
    pub dependency: Dependency,
}

/// Identifies a specific version of a crate.
#[derive(Debug, Clone)]
pub struct CrateVersion {
    pub crate_name: String,
    pub num: String,
}

// ── Response types ──────────────────────────────────────────────────────────

/// Response from `GET /crates/{name}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateResponse {
    #[serde(rename = "crate")]
    pub crate_data: Crate,
    pub versions: Vec<Version>,
}

/// Response from `GET /crates` (search).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CratesPage {
    pub crates: Vec<Crate>,
    pub meta: Meta,
}

/// Response from `GET /crates/{name}/downloads`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateDownloads {
    pub version_downloads: Vec<VersionDownloads>,
}

/// Reverse dependencies with pagination metadata.
#[derive(Debug, Clone)]
pub struct ReverseDependencies {
    pub dependencies: Vec<ReverseDependency>,
    pub meta: Meta,
}

/// Summary statistics from crates.io.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub num_crates: u64,
    pub num_downloads: u64,
    pub new_crates: Vec<Crate>,
    pub most_downloaded: Vec<Crate>,
    pub just_updated: Vec<Crate>,
    pub popular_keywords: Vec<Keyword>,
    pub popular_categories: Vec<Category>,
}

/// Paginated versions response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionsPage {
    pub versions: Vec<Version>,
    pub meta: Meta,
}

/// Paginated categories response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoriesPage {
    pub categories: Vec<Category>,
    pub meta: Meta,
}

/// Paginated keywords response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordsPage {
    pub keywords: Vec<Keyword>,
    pub meta: Meta,
}

// ── Team type ───────────────────────────────────────────────────────────────

/// A crates.io team.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

// ── User stats ──────────────────────────────────────────────────────────────

/// Download statistics for a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStats {
    pub total_downloads: u64,
}

// ── Category slugs ──────────────────────────────────────────────────────────

/// A minimal category entry from the category_slugs endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySlug {
    pub id: String,
    pub slug: String,
    #[serde(default)]
    pub description: Option<String>,
}

// ── Site metadata ───────────────────────────────────────────────────────────

/// Site deployment metadata from crates.io.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteMetadata {
    pub deployed_sha: Option<String>,
    #[serde(default)]
    pub commit: Option<String>,
}

// ── Authenticated types ─────────────────────────────────────────────────────

/// Generic ok/error response from mutation endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OkResponse {
    pub ok: bool,
}

/// Response from `GET /crates/{name}/following`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowingResponse {
    pub following: bool,
}

/// Settings for updating a crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
}

/// Settings for updating a version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yanked: Option<bool>,
}

// ── Owner types ─────────────────────────────────────────────────────────────

/// An invitation to become an owner of a crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerInvitation {
    pub invited_by_username: String,
    pub crate_name: String,
    pub crate_id: u64,
    pub created_at: DateTime<Utc>,
}

// ── API token types ─────────────────────────────────────────────────────────

/// An API token on crates.io.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiToken {
    pub id: u64,
    pub name: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub last_used_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub crate_scopes: Option<Vec<String>>,
    #[serde(default)]
    pub endpoint_scopes: Option<Vec<String>>,
}

// ── Publish types ───────────────────────────────────────────────────────────

/// Metadata for publishing a crate (JSON portion of PUT /crates/new).
#[derive(Debug, Clone, Serialize)]
pub struct PublishMetadata {
    pub name: String,
    #[serde(rename = "vers")]
    pub version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<PublishDependency>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rust_version: Option<String>,
}

/// A dependency entry in the publish metadata.
#[derive(Debug, Clone, Serialize)]
pub struct PublishDependency {
    pub name: String,
    pub version_req: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub default_features: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// "normal", "dev", or "build"
    #[serde(default)]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explicit_name_in_toml: Option<String>,
}

/// Warnings returned from the publish endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct PublishWarnings {
    #[serde(default)]
    pub invalid_categories: Vec<String>,
    #[serde(default)]
    pub invalid_badges: Vec<String>,
    #[serde(default)]
    pub other: Vec<String>,
}

// ── Trusted publishing types ────────────────────────────────────────────────

/// A GitHub trusted publishing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubConfig {
    pub id: u64,
    pub crate_name: String,
    pub repository_owner: String,
    pub repository_name: String,
    #[serde(default)]
    pub workflow_filename: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Input for creating a new GitHub trusted publishing config.
#[derive(Debug, Clone, Serialize)]
pub struct NewGitHubConfig {
    pub crate_name: String,
    pub repository_owner: String,
    pub repository_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
}

/// A GitLab trusted publishing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLabConfig {
    pub id: u64,
    pub crate_name: String,
    pub project_path: String,
    #[serde(default)]
    pub environment: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Input for creating a new GitLab trusted publishing config.
#[derive(Debug, Clone, Serialize)]
pub struct NewGitLabConfig {
    pub crate_name: String,
    pub project_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
}
