//! API token management endpoints.

use super::CratesIoClient;
use super::error::Error;
use super::types::ApiToken;
use super::wire::{CreateTokenData, CreateTokenRequest, TokenResponse, TokensResponse};

impl CratesIoClient {
    /// List all API tokens for the authenticated user.
    ///
    /// Requires authentication.
    pub async fn list_tokens(&self) -> Result<Vec<ApiToken>, Error> {
        let resp: TokensResponse = self.get_json_auth("/me/tokens").await?;
        Ok(resp.api_tokens)
    }

    /// Create a new API token.
    ///
    /// Requires authentication.
    pub async fn create_token(
        &self,
        name: &str,
        crate_scopes: Option<Vec<String>>,
        endpoint_scopes: Option<Vec<String>>,
    ) -> Result<ApiToken, Error> {
        let body = CreateTokenRequest {
            api_token: CreateTokenData {
                name: name.to_string(),
                crate_scopes,
                endpoint_scopes,
            },
        };
        let resp: TokenResponse = self.put_json("/me/tokens", &body).await?;
        Ok(resp.api_token)
    }

    /// Get details of a specific API token.
    ///
    /// Requires authentication.
    pub async fn get_token(&self, id: u64) -> Result<ApiToken, Error> {
        let resp: TokenResponse = self.get_json_auth(&format!("/me/tokens/{id}")).await?;
        Ok(resp.api_token)
    }

    /// Revoke (delete) a specific API token.
    ///
    /// Requires authentication.
    pub async fn revoke_token(&self, id: u64) -> Result<(), Error> {
        self.delete_ok(&format!("/me/tokens/{id}")).await
    }

    /// Revoke the token currently being used for authentication.
    ///
    /// Requires authentication.
    pub async fn revoke_current_token(&self) -> Result<(), Error> {
        self.delete_ok("/tokens/current").await
    }
}
