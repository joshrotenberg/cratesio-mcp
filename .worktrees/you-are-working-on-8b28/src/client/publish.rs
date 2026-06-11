//! Crate publishing endpoint.
//!
//! The `PUT /crates/new` endpoint uses a special binary format:
//! - 4 bytes LE: length of JSON metadata
//! - N bytes: JSON metadata
//! - 4 bytes LE: length of tarball
//! - M bytes: `.crate` tarball

use super::CratesIoClient;
use super::error::Error;
use super::types::{PublishMetadata, PublishWarnings};
use super::wire::PublishResponse;

impl CratesIoClient {
    /// Publish a new crate version.
    ///
    /// `metadata` is the JSON publish metadata, `tarball` is the `.crate` file bytes.
    ///
    /// Requires authentication.
    pub async fn publish(
        &self,
        metadata: &PublishMetadata,
        tarball: &[u8],
    ) -> Result<PublishWarnings, Error> {
        let json_bytes = serde_json::to_vec(metadata)?;

        // Build the binary body: json_len (4 LE) + json + tarball_len (4 LE) + tarball
        let mut body = Vec::with_capacity(4 + json_bytes.len() + 4 + tarball.len());
        body.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        body.extend_from_slice(&json_bytes);
        body.extend_from_slice(&(tarball.len() as u32).to_le_bytes());
        body.extend_from_slice(tarball);

        let resp: PublishResponse = self
            .put_bytes_json("/crates/new", body, "application/octet-stream")
            .await?;
        Ok(resp.warnings)
    }
}
