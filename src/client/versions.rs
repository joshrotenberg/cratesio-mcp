//! Version-related API endpoints.

use super::CratesIoClient;
use super::error::Error;
use super::types::{
    Authors, CrateDownloads, Dependency, OkResponse, Version, VersionSettings, VersionsPage,
};
use super::wire::{AuthorsResponse, DependenciesResponse, UpdateVersionRequest, VersionResponse};

impl CratesIoClient {
    /// Get paginated version list for a crate.
    pub async fn crate_versions(
        &self,
        name: &str,
        page: Option<u64>,
        per_page: Option<u64>,
    ) -> Result<VersionsPage, Error> {
        let mut params: Vec<(String, String)> = Vec::new();
        if let Some(page) = page {
            params.push(("page".into(), page.to_string()));
        }
        if let Some(per_page) = per_page {
            params.push(("per_page".into(), per_page.to_string()));
        }
        self.get_json_query(&format!("/crates/{name}/versions"), &params)
            .await
    }

    /// Get metadata for a specific crate version.
    pub async fn crate_version(&self, name: &str, version: &str) -> Result<Version, Error> {
        let resp: VersionResponse = self.get_json(&format!("/crates/{name}/{version}")).await?;
        Ok(resp.version)
    }

    /// Get per-day download data for a specific crate version.
    pub async fn version_downloads(
        &self,
        name: &str,
        version: &str,
    ) -> Result<CrateDownloads, Error> {
        self.get_json(&format!("/crates/{name}/{version}/downloads"))
            .await
    }

    /// Get dependencies for a specific crate version.
    pub async fn crate_dependencies(
        &self,
        name: &str,
        version: &str,
    ) -> Result<Vec<Dependency>, Error> {
        let resp: DependenciesResponse = self
            .get_json(&format!("/crates/{name}/{version}/dependencies"))
            .await?;
        Ok(resp.dependencies)
    }

    /// Get the rendered readme for a specific crate version.
    pub async fn crate_readme(&self, name: &str, version: &str) -> Result<String, Error> {
        self.get_text(&format!("/crates/{name}/{version}/readme"))
            .await
    }

    /// Get authors for a specific crate version.
    pub async fn crate_authors(&self, name: &str, version: &str) -> Result<Authors, Error> {
        let resp: AuthorsResponse = self
            .get_json(&format!("/crates/{name}/{version}/authors"))
            .await?;
        Ok(Authors {
            names: resp.meta.names,
        })
    }

    // ── Authenticated endpoints ─────────────────────────────────────────

    /// Yank a specific version.
    ///
    /// Requires authentication.
    pub async fn yank_version(&self, name: &str, version: &str) -> Result<OkResponse, Error> {
        self.delete_json(&format!("/crates/{name}/{version}/yank"))
            .await
    }

    /// Unyank a previously yanked version.
    ///
    /// Requires authentication.
    pub async fn unyank_version(&self, name: &str, version: &str) -> Result<OkResponse, Error> {
        self.put_empty(&format!("/crates/{name}/{version}/unyank"))
            .await
    }

    /// Update version settings (currently only yank status).
    ///
    /// Requires authentication.
    pub async fn update_version(
        &self,
        name: &str,
        version: &str,
        settings: VersionSettings,
    ) -> Result<Version, Error> {
        let body = UpdateVersionRequest { version: settings };
        let resp: VersionResponse = self
            .patch_json(&format!("/crates/{name}/{version}"), &body)
            .await?;
        Ok(resp.version)
    }
}
