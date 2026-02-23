//! User-related API endpoints.

use super::CratesIoClient;
use super::error::Error;
use super::types::{User, UserStats, Version};
use super::wire::{
    MyUpdatesResponse, UpdateUserData, UpdateUserRequest, UserResponse, UserStatsResponse,
};

impl CratesIoClient {
    /// Get a user's profile by GitHub username.
    pub async fn user(&self, username: &str) -> Result<User, Error> {
        let resp: UserResponse = self.get_json(&format!("/users/{username}")).await?;
        Ok(resp.user)
    }

    /// Get download statistics for a user.
    pub async fn user_stats(&self, user_id: u64) -> Result<UserStats, Error> {
        let resp: UserStatsResponse = self.get_json(&format!("/users/{user_id}/stats")).await?;
        Ok(UserStats {
            total_downloads: resp.total_downloads,
        })
    }

    // ── Authenticated endpoints ─────────────────────────────────────────

    /// Get the authenticated user's profile.
    ///
    /// Requires authentication.
    pub async fn me(&self) -> Result<User, Error> {
        let resp: UserResponse = self.get_json_auth("/me").await?;
        Ok(resp.user)
    }

    /// Update a user's email address.
    ///
    /// Requires authentication.
    pub async fn update_user(&self, user_id: u64, email: Option<String>) -> Result<(), Error> {
        let body = UpdateUserRequest {
            user: UpdateUserData { email },
        };
        self.put_json_ok(&format!("/users/{user_id}"), &body).await
    }

    /// Get the authenticated user's followed crate updates.
    ///
    /// Requires authentication. Returns versions and whether there are more pages.
    pub async fn my_updates(
        &self,
        page: Option<u64>,
        per_page: Option<u64>,
    ) -> Result<(Vec<Version>, bool), Error> {
        let mut params: Vec<(String, String)> = Vec::new();
        if let Some(page) = page {
            params.push(("page".into(), page.to_string()));
        }
        if let Some(per_page) = per_page {
            params.push(("per_page".into(), per_page.to_string()));
        }
        let resp: MyUpdatesResponse = self.get_json_query_auth("/me/updates", &params).await?;
        Ok((resp.versions, resp.meta.more))
    }
}
