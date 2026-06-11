//! Query builder for crate search.

/// Sort order for crate search results.
#[derive(Debug, Clone, Copy)]
pub enum Sort {
    Alphabetical,
    Relevance,
    Downloads,
    RecentDownloads,
    RecentUpdates,
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
    pub fn search(mut self, search: &str) -> Self {
        self.query.search = Some(search.to_string());
        self
    }

    pub fn sort(mut self, sort: Sort) -> Self {
        self.query.sort = Some(sort);
        self
    }

    pub fn page(mut self, page: u64) -> Self {
        self.query.page = Some(page);
        self
    }

    pub fn per_page(mut self, per_page: u64) -> Self {
        self.query.per_page = Some(per_page);
        self
    }

    pub fn build(self) -> CratesQuery {
        self.query
    }
}
