//! OSV.dev API client for vulnerability lookups.
//!
//! Queries the [OSV.dev](https://osv.dev/) API to check Rust crates for known
//! security vulnerabilities aggregated from RustSec, GHSA, and NVD.

use serde::{Deserialize, Serialize};

// ── Error ──────────────────────────────────────────────────────────────────

/// Errors returned by the OSV.dev API client.
#[derive(Debug, thiserror::Error)]
pub enum OsvError {
    /// HTTP transport error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Non-200 response from the API.
    #[error("OSV API error ({status}): {message}")]
    Api { status: u16, message: String },
}

// ── Response types ─────────────────────────────────────────────────────────

/// Top-level response from `POST /v1/query`.
#[derive(Debug, Deserialize)]
pub struct OsvQueryResponse {
    pub vulns: Option<Vec<OsvVulnerability>>,
}

/// A single vulnerability record.
#[derive(Debug, Deserialize)]
pub struct OsvVulnerability {
    /// Advisory ID (e.g. "RUSTSEC-2021-0078", "GHSA-...").
    pub id: String,
    pub summary: Option<String>,
    pub details: Option<String>,
    pub severity: Option<Vec<OsvSeverity>>,
    pub affected: Option<Vec<OsvAffected>>,
    pub references: Option<Vec<OsvReference>>,
}

/// CVSS severity information.
#[derive(Debug, Deserialize)]
pub struct OsvSeverity {
    /// Severity scheme (e.g. "CVSS_V3", "CVSS_V4").
    #[serde(rename = "type")]
    pub severity_type: String,
    /// CVSS vector string.
    pub score: String,
}

/// Affected package and version range info.
#[derive(Debug, Deserialize)]
pub struct OsvAffected {
    pub package: Option<OsvPackage>,
    pub ranges: Option<Vec<OsvRange>>,
}

/// Package identifier within an ecosystem.
#[derive(Debug, Deserialize)]
pub struct OsvPackage {
    pub name: String,
    pub ecosystem: String,
}

/// A version range that is affected.
#[derive(Debug, Deserialize)]
pub struct OsvRange {
    #[serde(rename = "type")]
    pub range_type: String,
    pub events: Vec<OsvEvent>,
}

/// A version event (introduced/fixed boundary).
#[derive(Debug, Deserialize)]
pub struct OsvEvent {
    pub introduced: Option<String>,
    pub fixed: Option<String>,
}

/// A reference link (advisory URL, etc).
#[derive(Debug, Deserialize)]
pub struct OsvReference {
    #[serde(rename = "type")]
    pub ref_type: String,
    pub url: String,
}

// ── Request body ───────────────────────────────────────────────────────────

#[derive(Serialize)]
struct OsvQueryRequest<'a> {
    package: OsvPackageQuery<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
}

#[derive(Serialize)]
struct OsvPackageQuery<'a> {
    name: &'a str,
    ecosystem: &'a str,
}

// ── Client ─────────────────────────────────────────────────────────────────

/// Async client for the OSV.dev vulnerability API.
pub struct OsvClient {
    http: reqwest::Client,
    base_url: String,
}

impl OsvClient {
    /// Create a new client with the given user agent.
    pub fn new(user_agent: &str) -> Result<Self, OsvError> {
        Self::with_base_url(user_agent, "https://api.osv.dev/v1")
    }

    /// Create a new client with a custom base URL (for testing).
    pub fn with_base_url(user_agent: &str, base_url: &str) -> Result<Self, OsvError> {
        let http = reqwest::Client::builder().user_agent(user_agent).build()?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    /// Query OSV for vulnerabilities affecting a specific package version.
    pub async fn query_package(
        &self,
        name: &str,
        version: &str,
    ) -> Result<OsvQueryResponse, OsvError> {
        let body = OsvQueryRequest {
            package: OsvPackageQuery {
                name,
                ecosystem: "crates.io",
            },
            version: Some(version),
        };
        self.post_query(&body).await
    }

    /// Query OSV for all known vulnerabilities for a package (any version).
    pub async fn query_package_any(&self, name: &str) -> Result<OsvQueryResponse, OsvError> {
        let body = OsvQueryRequest {
            package: OsvPackageQuery {
                name,
                ecosystem: "crates.io",
            },
            version: None,
        };
        self.post_query(&body).await
    }

    async fn post_query(&self, body: &OsvQueryRequest<'_>) -> Result<OsvQueryResponse, OsvError> {
        let url = format!("{}/query", self.base_url);
        let resp = self.http.post(&url).json(body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(OsvError::Api {
                status: status.as_u16(),
                message,
            });
        }
        Ok(resp.json().await?)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn test_client(base_url: &str) -> OsvClient {
        OsvClient::with_base_url("test-agent", base_url).unwrap()
    }

    #[tokio::test]
    async fn query_returns_vulnerabilities() {
        let server = MockServer::start().await;

        let response_json = serde_json::json!({
            "vulns": [{
                "id": "RUSTSEC-2024-0001",
                "summary": "Test vulnerability",
                "details": "A test vulnerability for unit testing.",
                "severity": [{
                    "type": "CVSS_V3",
                    "score": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:N/A:N"
                }],
                "affected": [{
                    "package": {
                        "name": "some-crate",
                        "ecosystem": "crates.io"
                    },
                    "ranges": [{
                        "type": "SEMVER",
                        "events": [
                            { "introduced": "0" },
                            { "fixed": "1.2.3" }
                        ]
                    }]
                }],
                "references": [{
                    "type": "ADVISORY",
                    "url": "https://rustsec.org/advisories/RUSTSEC-2024-0001.html"
                }]
            }]
        });

        Mock::given(method("POST"))
            .and(path("/query"))
            .and(body_json(serde_json::json!({
                "package": { "name": "some-crate", "ecosystem": "crates.io" },
                "version": "1.0.0"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_json))
            .expect(1)
            .mount(&server)
            .await;

        let client = test_client(&server.uri());
        let resp = client.query_package("some-crate", "1.0.0").await.unwrap();

        let vulns = resp.vulns.unwrap();
        assert_eq!(vulns.len(), 1);
        assert_eq!(vulns[0].id, "RUSTSEC-2024-0001");
        assert_eq!(vulns[0].summary.as_deref(), Some("Test vulnerability"));

        let severity = vulns[0].severity.as_ref().unwrap();
        assert_eq!(severity[0].severity_type, "CVSS_V3");

        let affected = vulns[0].affected.as_ref().unwrap();
        let ranges = affected[0].ranges.as_ref().unwrap();
        let events = &ranges[0].events;
        assert_eq!(events[0].introduced.as_deref(), Some("0"));
        assert_eq!(events[1].fixed.as_deref(), Some("1.2.3"));
    }

    #[tokio::test]
    async fn query_returns_no_vulnerabilities() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;

        let client = test_client(&server.uri());
        let resp = client.query_package("safe-crate", "1.0.0").await.unwrap();

        assert!(resp.vulns.is_none());
    }

    #[tokio::test]
    async fn query_handles_api_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/query"))
            .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
            .expect(1)
            .mount(&server)
            .await;

        let client = test_client(&server.uri());
        let err = client
            .query_package("bad-crate", "1.0.0")
            .await
            .unwrap_err();

        match err {
            OsvError::Api { status, message } => {
                assert_eq!(status, 400);
                assert_eq!(message, "bad request");
            }
            other => panic!("expected Api error, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn query_any_omits_version() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/query"))
            .and(body_json(serde_json::json!({
                "package": { "name": "some-crate", "ecosystem": "crates.io" }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "vulns": []
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = test_client(&server.uri());
        let resp = client.query_package_any("some-crate").await.unwrap();

        assert!(resp.vulns.unwrap().is_empty());
    }
}
