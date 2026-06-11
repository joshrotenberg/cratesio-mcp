//! Team-related API endpoints.

use super::CratesIoClient;
use super::error::Error;
use super::types::Team;
use super::wire::TeamResponse;

impl CratesIoClient {
    /// Get a team by login (e.g. `github:org:team-name`).
    pub async fn team(&self, login: &str) -> Result<Team, Error> {
        let resp: TeamResponse = self.get_json(&format!("/teams/{login}")).await?;
        Ok(resp.team)
    }
}
