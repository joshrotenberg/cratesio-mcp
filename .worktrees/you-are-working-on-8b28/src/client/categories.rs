//! Category-related API endpoints.

use super::CratesIoClient;
use super::error::Error;
use super::types::{CategoriesPage, Category, CategorySlug};
use super::wire::{CategoryResponse, CategorySlugsResponse};

impl CratesIoClient {
    /// Get paginated list of all categories.
    pub async fn categories(
        &self,
        page: Option<u64>,
        per_page: Option<u64>,
    ) -> Result<CategoriesPage, Error> {
        let mut params: Vec<(String, String)> = Vec::new();
        if let Some(page) = page {
            params.push(("page".into(), page.to_string()));
        }
        if let Some(per_page) = per_page {
            params.push(("per_page".into(), per_page.to_string()));
        }
        self.get_json_query("/categories", &params).await
    }

    /// Get a single category by slug.
    pub async fn category(&self, slug: &str) -> Result<Category, Error> {
        let resp: CategoryResponse = self.get_json(&format!("/categories/{slug}")).await?;
        Ok(resp.category)
    }

    /// Get all category slugs (lightweight listing).
    pub async fn category_slugs(&self) -> Result<Vec<CategorySlug>, Error> {
        let resp: CategorySlugsResponse = self.get_json("/category_slugs").await?;
        Ok(resp.category_slugs)
    }
}
