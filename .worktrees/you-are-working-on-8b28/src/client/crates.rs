//! Crate-related API endpoints.

use std::collections::HashMap;

use super::CratesIoClient;
use super::error::Error;
use super::query::CratesQuery;
use super::types::{
    CrateDownloads, CrateResponse, CrateSettings, CratesPage, FollowingResponse, OkResponse,
    ReverseDependencies, ReverseDependency, Summary,
};
use super::wire::{ReverseDependenciesRaw, UpdateCrateRequest};
use crate::client::types::CrateVersion;

impl CratesIoClient {
    /// Get crates.io summary statistics.
    pub async fn summary(&self) -> Result<Summary, Error> {
        self.get_json("/summary").await
    }

    /// Search for crates.
    pub async fn crates(&self, query: CratesQuery) -> Result<CratesPage, Error> {
        let mut params: Vec<(String, String)> = Vec::new();
        if let Some(search) = query.search {
            params.push(("q".into(), search));
        }
        if let Some(sort) = query.sort {
            params.push(("sort".into(), sort.as_str().into()));
        }
        if let Some(page) = query.page {
            params.push(("page".into(), page.to_string()));
        }
        if let Some(per_page) = query.per_page {
            params.push(("per_page".into(), per_page.to_string()));
        }
        self.get_json_query("/crates", &params).await
    }

    /// Get detailed information about a crate.
    pub async fn get_crate(&self, name: &str) -> Result<CrateResponse, Error> {
        self.get_json(&format!("/crates/{name}")).await
    }

    /// Get download statistics for a crate (last 90 days, all versions).
    pub async fn crate_downloads(&self, name: &str) -> Result<CrateDownloads, Error> {
        self.get_json(&format!("/crates/{name}/downloads")).await
    }

    /// Get reverse dependencies (crates that depend on this crate).
    pub async fn crate_reverse_dependencies(
        &self,
        name: &str,
    ) -> Result<ReverseDependencies, Error> {
        let raw: ReverseDependenciesRaw = self
            .get_json(&format!("/crates/{name}/reverse_dependencies"))
            .await?;

        // Build a lookup from version ID to (crate_name, version_num)
        let version_map: HashMap<u64, (String, String)> = raw
            .versions
            .into_iter()
            .map(|v| (v.id, (v.krate, v.num)))
            .collect();

        // Join dependencies with their version info
        let dependencies = raw
            .dependencies
            .into_iter()
            .filter_map(|dep| {
                let version_id = dep.version_id;
                version_map
                    .get(&version_id)
                    .map(|(crate_name, num)| ReverseDependency {
                        crate_version: CrateVersion {
                            crate_name: crate_name.clone(),
                            num: num.clone(),
                        },
                        dependency: dep,
                    })
            })
            .collect();

        Ok(ReverseDependencies {
            dependencies,
            meta: raw.meta,
        })
    }

    // ── Authenticated endpoints ─────────────────────────────────────────

    /// Update crate settings (description, docs, homepage, repository).
    ///
    /// Requires authentication.
    pub async fn update_crate(
        &self,
        name: &str,
        settings: CrateSettings,
    ) -> Result<CrateResponse, Error> {
        let body = UpdateCrateRequest {
            crate_data: settings,
        };
        self.patch_json(&format!("/crates/{name}"), &body).await
    }

    /// Delete a crate (only if it has a single owner and no dependents).
    ///
    /// Requires authentication.
    pub async fn delete_crate(&self, name: &str) -> Result<(), Error> {
        self.delete_ok(&format!("/crates/{name}")).await
    }

    /// Follow a crate to receive notifications.
    ///
    /// Requires authentication.
    pub async fn follow_crate(&self, name: &str) -> Result<OkResponse, Error> {
        self.put_empty(&format!("/crates/{name}/follow")).await
    }

    /// Unfollow a crate.
    ///
    /// Requires authentication.
    pub async fn unfollow_crate(&self, name: &str) -> Result<OkResponse, Error> {
        self.delete_json(&format!("/crates/{name}/follow")).await
    }

    /// Check if the authenticated user follows a crate.
    ///
    /// Requires authentication.
    pub async fn is_following(&self, name: &str) -> Result<bool, Error> {
        let resp: FollowingResponse = self
            .get_json_auth(&format!("/crates/{name}/following"))
            .await?;
        Ok(resp.following)
    }
}
