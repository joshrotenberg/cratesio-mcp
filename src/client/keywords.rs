//! Keyword-related API endpoints.

use super::CratesIoClient;
use super::error::Error;
use super::types::{Keyword, KeywordsPage};
use super::wire::KeywordResponse;

impl CratesIoClient {
    /// Get paginated list of all keywords.
    pub async fn keywords(
        &self,
        page: Option<u64>,
        per_page: Option<u64>,
    ) -> Result<KeywordsPage, Error> {
        let mut params: Vec<(String, String)> = Vec::new();
        if let Some(page) = page {
            params.push(("page".into(), page.to_string()));
        }
        if let Some(per_page) = per_page {
            params.push(("per_page".into(), per_page.to_string()));
        }
        self.get_json_query("/keywords", &params).await
    }

    /// Get a single keyword by ID.
    pub async fn keyword(&self, id: &str) -> Result<Keyword, Error> {
        let resp: KeywordResponse = self.get_json(&format!("/keywords/{id}")).await?;
        Ok(resp.keyword)
    }
}
