//! Query builder for crate search.

/// Sort order for crate search results.
#[derive(Debug, Clone, Copy)]
pub enum Sort {
    /// Sort alphabetically by crate name.
    Alphabetical,
    /// Sort by relevance to the search query.
    Relevance,
    /// Sort by all-time download count.
    Downloads,
    /// Sort by recent download count.
    RecentDownloads,
    /// Sort by most recently updated.
    RecentUpdates,
    /// Sort by most recently added.
    NewlyAdded,
}

impl Sort {
    pub(crate) fn as_str(&self) -> &str {
        match self {
            Sort::Alphabetical => "alpha",
            Sort::Relevance => "relevance",
            Sort::Downloads => "downloads",
            Sort::RecentDownloads => "recent-downloads",
            Sort::RecentUpdates => "recent-updates",
            Sort::NewlyAdded => "new",
        }
    }
}

/// Query parameters for crate search.
#[derive(Debug, Clone, Default)]
pub struct CratesQuery {
    pub(crate) search: Option<String>,
    pub(crate) sort: Option<Sort>,
    pub(crate) page: Option<u64>,
    pub(crate) per_page: Option<u64>,
}

impl CratesQuery {
    /// Create a new query builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use cratesio_mcp::client::{CratesQuery, Sort};
    ///
    /// let query = CratesQuery::builder()
    ///     .search("tower")
    ///     .sort(Sort::Downloads)
    ///     .build();
    /// ```
    pub fn builder() -> CratesQueryBuilder {
        CratesQueryBuilder {
            query: CratesQuery::default(),
        }
    }
}

/// Builder for [`CratesQuery`].
pub struct CratesQueryBuilder {
    query: CratesQuery,
}

impl CratesQueryBuilder {
    /// Set the search term to filter crates by.
    pub fn search(mut self, search: &str) -> Self {
        self.query.search = Some(search.to_string());
        self
    }

    /// Set the sort order for the results.
    pub fn sort(mut self, sort: Sort) -> Self {
        self.query.sort = Some(sort);
        self
    }

    /// Set the page number to fetch (1-based).
    pub fn page(mut self, page: u64) -> Self {
        self.query.page = Some(page);
        self
    }

    /// Set the number of results to return per page.
    pub fn per_page(mut self, per_page: u64) -> Self {
        self.query.per_page = Some(per_page);
        self
    }

    /// Finalize the builder and produce a [`CratesQuery`].
    pub fn build(self) -> CratesQuery {
        self.query
    }
}
