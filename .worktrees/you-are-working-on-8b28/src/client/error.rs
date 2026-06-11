//! Error types for the crates.io API client.

/// Errors returned by the crates.io API client.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// HTTP transport error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Resource not found (404).
    #[error("not found: {0}")]
    NotFound(String),

    /// Permission denied (403).
    #[error("permission denied")]
    PermissionDenied,

    /// Server returned an error status.
    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    /// Rate limited by the server (429).
    #[error("rate limited")]
    RateLimited,

    /// Authentication required for this endpoint.
    #[error("authentication required")]
    AuthRequired,

    /// Unauthorized (401).
    #[error("unauthorized")]
    Unauthorized,

    /// JSON serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
