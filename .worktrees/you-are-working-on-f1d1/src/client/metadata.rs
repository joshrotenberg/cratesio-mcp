//! Site metadata endpoints.

use super::CratesIoClient;
use super::error::Error;
use super::types::SiteMetadata;
use super::wire::SiteMetadataResponse;

impl CratesIoClient {
    /// Get site deployment metadata.
    pub async fn site_metadata(&self) -> Result<SiteMetadata, Error> {
        let resp: SiteMetadataResponse = self.get_json("/site_metadata").await?;
        Ok(SiteMetadata {
            deployed_sha: resp.deployed_sha,
            commit: resp.commit,
        })
    }
}
