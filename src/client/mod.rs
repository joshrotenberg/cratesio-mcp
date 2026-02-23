//! Custom crates.io API client
//!
//! Async client for the crates.io REST API, built on reqwest with built-in
//! rate limiting. Supports both anonymous and authenticated access.

pub mod docsrs;
pub mod error;
pub mod osv;
pub mod query;
pub mod types;
pub(crate) mod wire;

mod categories;
mod crates;
mod keywords;
mod metadata;
mod owners;
mod publish;
mod teams;
mod tokens;
mod trusted;
mod users;
mod versions;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;
use tokio::time::Instant;

pub use error::Error;
pub use query::{CratesQuery, CratesQueryBuilder, Sort};
pub use types::*;

// ── Auth ────────────────────────────────────────────────────────────────────

/// Authentication credentials for the crates.io API.
struct Auth {
    token: String,
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Auth")
            .field("token", &"[REDACTED]")
            .finish()
    }
}

// ── Client ──────────────────────────────────────────────────────────────────

/// Async client for the crates.io REST API.
///
/// Includes built-in rate limiting to respect the crates.io crawling policy.
/// Supports optional authentication via API token for write operations.
pub struct CratesIoClient {
    http: reqwest::Client,
    base_url: String,
    rate_limit: Duration,
    last_request: Arc<Mutex<Option<Instant>>>,
    auth: Option<Auth>,
}

impl CratesIoClient {
    /// Create a new client with the given user agent and rate limit.
    pub fn new(user_agent: &str, rate_limit: Duration) -> Result<Self, Error> {
        Self::with_base_url(user_agent, rate_limit, "https://crates.io/api/v1")
    }

    /// Create a new client with a custom base URL (for testing).
    pub fn with_base_url(
        user_agent: &str,
        rate_limit: Duration,
        base_url: &str,
    ) -> Result<Self, Error> {
        let http = reqwest::Client::builder().user_agent(user_agent).build()?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            rate_limit,
            last_request: Arc::new(Mutex::new(None)),
            auth: None,
        })
    }

    /// Enable authentication with an API token.
    ///
    /// Returns `self` for builder-style chaining.
    pub fn with_auth(mut self, token: impl Into<String>) -> Self {
        self.auth = Some(Auth {
            token: token.into(),
        });
        self
    }

    /// Returns the auth token or `Error::AuthRequired`.
    pub(crate) fn require_auth(&self) -> Result<&str, Error> {
        self.auth
            .as_ref()
            .map(|a| a.token.as_str())
            .ok_or(Error::AuthRequired)
    }

    // ── Unauthenticated HTTP helpers ────────────────────────────────────

    /// Enforce rate limiting between requests.
    pub(crate) async fn throttle(&self) {
        let mut last = self.last_request.lock().await;
        if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            if elapsed < self.rate_limit {
                tokio::time::sleep(self.rate_limit - elapsed).await;
            }
        }
        *last = Some(Instant::now());
    }

    /// Send a GET request and check the response status.
    pub(crate) async fn send(&self, path: &str) -> Result<reqwest::Response, Error> {
        self.throttle().await;
        let url = format!("{}{}", self.base_url, path);
        let resp = self.http.get(&url).send().await?;
        Self::check_status(resp, path).await
    }

    /// Send a GET request with query parameters and check the response status.
    pub(crate) async fn send_query(
        &self,
        path: &str,
        query: &[(String, String)],
    ) -> Result<reqwest::Response, Error> {
        self.throttle().await;
        let url = format!("{}{}", self.base_url, path);
        let resp = self.http.get(&url).query(query).send().await?;
        Self::check_status(resp, path).await
    }

    /// Map non-success HTTP status codes to typed errors.
    pub(crate) async fn check_status(
        resp: reqwest::Response,
        path: &str,
    ) -> Result<reqwest::Response, Error> {
        let status = resp.status();
        if status.is_success() {
            Ok(resp)
        } else if status == reqwest::StatusCode::NOT_FOUND {
            Err(Error::NotFound(path.to_string()))
        } else if status == reqwest::StatusCode::UNAUTHORIZED {
            Err(Error::Unauthorized)
        } else if status == reqwest::StatusCode::FORBIDDEN {
            Err(Error::PermissionDenied)
        } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            Err(Error::RateLimited)
        } else {
            let text = resp.text().await.unwrap_or_default();
            Err(Error::Api {
                status: status.as_u16(),
                message: text,
            })
        }
    }

    /// GET a JSON resource.
    pub(crate) async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let resp = self.send(path).await?;
        Ok(resp.json().await?)
    }

    /// GET a JSON resource with query parameters.
    pub(crate) async fn get_json_query<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(String, String)],
    ) -> Result<T, Error> {
        let resp = self.send_query(path, query).await?;
        Ok(resp.json().await?)
    }

    /// GET a text resource (e.g. readme).
    pub(crate) async fn get_text(&self, path: &str) -> Result<String, Error> {
        let resp = self.send(path).await?;
        Ok(resp.text().await?)
    }

    // ── Authenticated HTTP helpers ──────────────────────────────────────

    /// Send an authenticated GET request.
    pub(crate) async fn send_auth(&self, path: &str) -> Result<reqwest::Response, Error> {
        let token = self.require_auth()?;
        self.throttle().await;
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", token)
            .send()
            .await?;
        Self::check_status(resp, path).await
    }

    /// Send an authenticated GET request with query parameters.
    pub(crate) async fn send_query_auth(
        &self,
        path: &str,
        query: &[(String, String)],
    ) -> Result<reqwest::Response, Error> {
        let token = self.require_auth()?;
        self.throttle().await;
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", token)
            .query(query)
            .send()
            .await?;
        Self::check_status(resp, path).await
    }

    /// GET a JSON resource with authentication.
    pub(crate) async fn get_json_auth<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let resp = self.send_auth(path).await?;
        Ok(resp.json().await?)
    }

    /// GET a JSON resource with query params and authentication.
    pub(crate) async fn get_json_query_auth<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(String, String)],
    ) -> Result<T, Error> {
        let resp = self.send_query_auth(path, query).await?;
        Ok(resp.json().await?)
    }

    /// PUT a JSON body and return a deserialized response. Requires auth.
    pub(crate) async fn put_json<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        let token = self.require_auth()?;
        self.throttle().await;
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .put(&url)
            .header("Authorization", token)
            .json(body)
            .send()
            .await?;
        let resp = Self::check_status(resp, path).await?;
        Ok(resp.json().await?)
    }

    /// PUT a JSON body, expecting no meaningful response body. Requires auth.
    pub(crate) async fn put_json_ok<B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<(), Error> {
        let token = self.require_auth()?;
        self.throttle().await;
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .put(&url)
            .header("Authorization", token)
            .json(body)
            .send()
            .await?;
        Self::check_status(resp, path).await?;
        Ok(())
    }

    /// PUT with no body, returning a deserialized JSON response. Requires auth.
    pub(crate) async fn put_empty<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let token = self.require_auth()?;
        self.throttle().await;
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .put(&url)
            .header("Authorization", token)
            .send()
            .await?;
        let resp = Self::check_status(resp, path).await?;
        Ok(resp.json().await?)
    }

    /// PUT with no body, returning deserialized JSON. No auth.
    pub(crate) async fn put_empty_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        self.throttle().await;
        let url = format!("{}{}", self.base_url, path);
        let resp = self.http.put(&url).send().await?;
        let resp = Self::check_status(resp, path).await?;
        Ok(resp.json().await?)
    }

    /// DELETE and return a deserialized JSON response. Requires auth.
    pub(crate) async fn delete_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let token = self.require_auth()?;
        self.throttle().await;
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .delete(&url)
            .header("Authorization", token)
            .send()
            .await?;
        let resp = Self::check_status(resp, path).await?;
        Ok(resp.json().await?)
    }

    /// DELETE with a JSON body and return deserialized response. Requires auth.
    pub(crate) async fn delete_json_with_body<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        let token = self.require_auth()?;
        self.throttle().await;
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .delete(&url)
            .header("Authorization", token)
            .json(body)
            .send()
            .await?;
        let resp = Self::check_status(resp, path).await?;
        Ok(resp.json().await?)
    }

    /// DELETE expecting no response body (just check status). Requires auth.
    pub(crate) async fn delete_ok(&self, path: &str) -> Result<(), Error> {
        let token = self.require_auth()?;
        self.throttle().await;
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .delete(&url)
            .header("Authorization", token)
            .send()
            .await?;
        Self::check_status(resp, path).await?;
        Ok(())
    }

    /// PATCH a JSON body and return deserialized response. Requires auth.
    pub(crate) async fn patch_json<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        let token = self.require_auth()?;
        self.throttle().await;
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .patch(&url)
            .header("Authorization", token)
            .json(body)
            .send()
            .await?;
        let resp = Self::check_status(resp, path).await?;
        Ok(resp.json().await?)
    }

    /// POST a JSON body and return deserialized response. Requires auth.
    pub(crate) async fn post_json<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        let token = self.require_auth()?;
        self.throttle().await;
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .post(&url)
            .header("Authorization", token)
            .json(body)
            .send()
            .await?;
        let resp = Self::check_status(resp, path).await?;
        Ok(resp.json().await?)
    }

    /// POST a JSON body without authentication and return deserialized response.
    pub(crate) async fn post_json_unauth<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        self.throttle().await;
        let url = format!("{}{}", self.base_url, path);
        let resp = self.http.post(&url).json(body).send().await?;
        let resp = Self::check_status(resp, path).await?;
        Ok(resp.json().await?)
    }

    /// PUT raw bytes with a custom content type and return deserialized JSON. Requires auth.
    pub(crate) async fn put_bytes_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<T, Error> {
        let token = self.require_auth()?;
        self.throttle().await;
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .put(&url)
            .header("Authorization", token)
            .header("Content-Type", content_type)
            .body(body)
            .send()
            .await?;
        let resp = Self::check_status(resp, path).await?;
        Ok(resp.json().await?)
    }
}
