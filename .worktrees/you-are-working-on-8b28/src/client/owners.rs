//! Crate owner management endpoints.

use super::CratesIoClient;
use super::error::Error;
use super::types::{OkResponse, OwnerInvitation, User};
use super::wire::{
    HandleInvitationRequest, InvitationTokenResponse, OwnerInvitationsResponse, OwnersRequest,
    OwnersResponse,
};

impl CratesIoClient {
    /// Get owners/maintainers of a crate.
    pub async fn crate_owners(&self, name: &str) -> Result<Vec<User>, Error> {
        let resp: OwnersResponse = self.get_json(&format!("/crates/{name}/owners")).await?;
        Ok(resp.users)
    }

    /// Get user owners of a crate.
    pub async fn crate_user_owners(&self, name: &str) -> Result<Vec<User>, Error> {
        let resp: OwnersResponse = self.get_json(&format!("/crates/{name}/owner_user")).await?;
        Ok(resp.users)
    }

    /// Get team owners of a crate.
    pub async fn crate_team_owners(&self, name: &str) -> Result<Vec<User>, Error> {
        let resp: OwnersResponse = self.get_json(&format!("/crates/{name}/owner_team")).await?;
        Ok(resp.users)
    }

    // ── Authenticated endpoints ─────────────────────────────────────────

    /// Add owners to a crate.
    ///
    /// Requires authentication. `logins` are GitHub usernames or team names.
    pub async fn add_owners(&self, name: &str, logins: Vec<String>) -> Result<OkResponse, Error> {
        let body = OwnersRequest { users: logins };
        self.put_json(&format!("/crates/{name}/owners"), &body)
            .await
    }

    /// Remove owners from a crate.
    ///
    /// Requires authentication. `logins` are GitHub usernames or team names.
    pub async fn remove_owners(
        &self,
        name: &str,
        logins: Vec<String>,
    ) -> Result<OkResponse, Error> {
        let body = OwnersRequest { users: logins };
        self.delete_json_with_body(&format!("/crates/{name}/owners"), &body)
            .await
    }

    /// List owner invitations for a specific crate.
    ///
    /// Requires authentication.
    pub async fn crate_owner_invitations(&self, name: &str) -> Result<Vec<OwnerInvitation>, Error> {
        let resp: OwnerInvitationsResponse = self
            .get_json_auth(&format!("/crates/{name}/owner_invitations"))
            .await?;
        Ok(resp.crate_owner_invitations)
    }

    /// List the authenticated user's pending owner invitations.
    ///
    /// Requires authentication.
    pub async fn my_owner_invitations(&self) -> Result<Vec<OwnerInvitation>, Error> {
        let resp: OwnerInvitationsResponse =
            self.get_json_auth("/me/crate_owner_invitations").await?;
        Ok(resp.crate_owner_invitations)
    }

    /// Accept or decline an owner invitation.
    ///
    /// Requires authentication.
    pub async fn handle_owner_invitation(
        &self,
        crate_id: u64,
        accept: bool,
    ) -> Result<OkResponse, Error> {
        let body = HandleInvitationRequest {
            accepted: accept,
            crate_id,
        };
        self.put_json(&format!("/me/crate_owner_invitations/{crate_id}"), &body)
            .await
    }

    /// Accept an owner invitation using a token (from email link).
    pub async fn accept_invitation_by_token(&self, token: &str) -> Result<u64, Error> {
        let resp: InvitationTokenResponse = self
            .put_empty_json(&format!("/crate_owner_invitations/{token}"))
            .await?;
        Ok(resp.crate_owner_invitation.crate_id)
    }
}
