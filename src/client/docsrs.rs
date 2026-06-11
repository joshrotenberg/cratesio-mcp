//! docs.rs API client for fetching rustdoc JSON.

use flate2::read::GzDecoder;
use rustdoc_types::Crate;
use std::io::Read;
use std::time::Duration;

/// Errors from the docs.rs client.
#[derive(Debug, thiserror::Error)]
pub enum DocsRsError {
    /// HTTP transport error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Crate or version not found on docs.rs.
    #[error("not found: {name} v{version}")]
    NotFound { name: String, version: String },

    /// Rustdoc JSON not available (crate predates docs.rs JSON support).
    #[error(
        "rustdoc JSON not available for {name} v{version} (requires docs.rs builds after 2025-05-23)"
    )]
    DocsNotAvailable { name: String, version: String },

    /// The docs.rs response exceeded the maximum allowed size.
    #[error("rustdoc JSON for {name} is too large ({size} bytes), exceeds the {limit} byte limit")]
    ResponseTooLarge { name: String, size: u64, limit: u64 },

    /// Failed to decompress gzip response from docs.rs.
    #[error("failed to decompress rustdoc JSON for {name}: {source}")]
    Decompress {
        name: String,
        source: std::io::Error,
    },

    /// Failed to parse the rustdoc JSON.
    #[error("failed to parse rustdoc JSON for {name}: {source}")]
    Parse {
        name: String,
        source: serde_json::Error,
    },

    /// Rustdoc JSON format version mismatch caused a parse failure.
    #[error(
        "failed to parse rustdoc JSON for {name}: docs.rs serves format v{actual} \
         but cratesio-mcp supports v{expected} -- consider updating the rustdoc-types dependency: {source}"
    )]
    FormatMismatch {
        name: String,
        expected: u32,
        actual: u32,
        source: serde_json::Error,
    },
}

/// Maximum number of bytes read or decompressed from a docs.rs response.
///
/// Caps both the compressed response body and the decompressed rustdoc JSON so
/// a malicious or pathologically large payload cannot exhaust memory.
const MAX_RESPONSE_BYTES: u64 = 100 * 1024 * 1024; // 100 MB

/// Minimal struct to extract just the format version from rustdoc JSON.
#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct FormatVersionCheck {
    format_version: u32,
}

/// Decode a docs.rs response body, enforcing `limit` on both the compressed
/// input and the decompressed output so a huge or malicious (gzip-bomb)
/// payload cannot exhaust memory.
fn decode_body(name: &str, bytes: &[u8], limit: u64) -> Result<Vec<u8>, DocsRsError> {
    // Servers may omit or understate Content-Length, so also enforce the
    // limit on the buffered compressed body.
    if bytes.len() as u64 > limit {
        return Err(DocsRsError::ResponseTooLarge {
            name: name.to_string(),
            size: bytes.len() as u64,
            limit,
        });
    }

    if bytes.starts_with(&[0x1f, 0x8b]) {
        let decoder = GzDecoder::new(bytes);
        // Read one byte past the limit so an over-limit payload is detected.
        let mut limited = decoder.take(limit + 1);
        let mut decompressed = Vec::new();
        limited
            .read_to_end(&mut decompressed)
            .map_err(|source| DocsRsError::Decompress {
                name: name.to_string(),
                source,
            })?;
        if decompressed.len() as u64 > limit {
            return Err(DocsRsError::ResponseTooLarge {
                name: name.to_string(),
                size: decompressed.len() as u64,
                limit,
            });
        }
        Ok(decompressed)
    } else {
        Ok(bytes.to_vec())
    }
}

/// HTTP client for the docs.rs rustdoc JSON API.
pub struct DocsRsClient {
    http: reqwest::Client,
    base_url: String,
}

impl DocsRsClient {
    /// Create a new client with the given user agent and outbound timeout.
    pub fn new(user_agent: &str, timeout: Duration) -> Result<Self, DocsRsError> {
        Self::with_base_url(user_agent, timeout, "https://docs.rs")
    }

    /// Create a new client with a custom base URL (for testing).
    pub fn with_base_url(
        user_agent: &str,
        timeout: Duration,
        base_url: &str,
    ) -> Result<Self, DocsRsError> {
        let http = reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(timeout)
            .build()?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    /// Fetch the rustdoc JSON for a crate version.
    ///
    /// The `version` parameter accepts `"latest"` or a specific semver string.
    pub async fn fetch_rustdoc(&self, name: &str, version: &str) -> Result<Crate, DocsRsError> {
        let url = format!("{}/crate/{}/{}/json.gz", self.base_url, name, version);
        let resp = self.http.get(&url).send().await?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(DocsRsError::NotFound {
                name: name.to_string(),
                version: version.to_string(),
            });
        }
        if status == reqwest::StatusCode::NOT_ACCEPTABLE {
            // docs.rs returns 406 when JSON is not available for a build
            return Err(DocsRsError::DocsNotAvailable {
                name: name.to_string(),
                version: version.to_string(),
            });
        }
        if !status.is_success() {
            // Map other errors to reqwest error via error_for_status
            let resp = resp.error_for_status()?;
            // unreachable but satisfy the compiler
            return Ok(resp.json().await?);
        }

        // Reject responses that advertise a size above the limit before
        // buffering the body, so a huge payload cannot exhaust memory.
        if let Some(len) = resp.content_length()
            && len > MAX_RESPONSE_BYTES
        {
            return Err(DocsRsError::ResponseTooLarge {
                name: name.to_string(),
                size: len,
                limit: MAX_RESPONSE_BYTES,
            });
        }

        let bytes = resp.bytes().await?;

        // docs.rs serves rustdoc JSON with Content-Type: application/gzip,
        // which reqwest does not auto-decompress (it only handles
        // Content-Encoding: gzip). Decompress manually, enforcing the size
        // limit on both the compressed body and the decompressed result.
        let json_bytes = decode_body(name, &bytes, MAX_RESPONSE_BYTES)?;

        // Pre-check format version before full deserialization.
        let actual_version = serde_json::from_slice::<FormatVersionCheck>(&json_bytes)
            .ok()
            .map(|c| c.format_version);

        let expected = rustdoc_types::FORMAT_VERSION;
        if let Some(actual) = actual_version
            && actual != expected
        {
            let diff = actual.abs_diff(expected);
            if diff <= 2 {
                tracing::warn!(
                    crate_name = name,
                    expected = expected,
                    actual = actual,
                    "rustdoc JSON format version mismatch (close): \
                     docs.rs serves v{actual}, we support v{expected}"
                );
            } else {
                tracing::warn!(
                    crate_name = name,
                    expected = expected,
                    actual = actual,
                    "rustdoc JSON format version mismatch (far): \
                     docs.rs serves v{actual}, we support v{expected}"
                );
            }
        }

        serde_json::from_slice(&json_bytes).map_err(|source| {
            if let Some(actual) = actual_version
                && actual != expected
            {
                return DocsRsError::FormatMismatch {
                    name: name.to_string(),
                    expected,
                    actual,
                    source,
                };
            }
            DocsRsError::Parse {
                name: name.to_string(),
                source,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn synthetic_crate_json() -> Vec<u8> {
        synthetic_crate_json_with_version(rustdoc_types::FORMAT_VERSION)
    }

    fn synthetic_crate_json_with_version(format_version: u32) -> Vec<u8> {
        let json = serde_json::json!({
            "root": 0,
            "crate_version": "1.0.0",
            "includes_private": false,
            "index": {},
            "paths": {},
            "external_crates": {},
            "target": {
                "triple": "x86_64-unknown-linux-gnu",
                "target_features": []
            },
            "format_version": format_version
        });
        serde_json::to_vec(&json).unwrap()
    }

    fn gzip_compress(data: &[u8]) -> Vec<u8> {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    #[tokio::test]
    async fn fetch_rustdoc_gzip_response() {
        let server = MockServer::start().await;
        let compressed = gzip_compress(&synthetic_crate_json());
        Mock::given(method("GET"))
            .and(path("/crate/serde/latest/json.gz"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(compressed)
                    .insert_header("content-type", "application/gzip"),
            )
            .mount(&server)
            .await;

        let client =
            DocsRsClient::with_base_url("test", Duration::from_secs(30), &server.uri()).unwrap();
        let krate = client.fetch_rustdoc("serde", "latest").await.unwrap();
        assert_eq!(krate.crate_version.as_deref(), Some("1.0.0"));
    }

    #[tokio::test]
    async fn fetch_rustdoc_plain_json_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crate/serde/latest/json.gz"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(synthetic_crate_json())
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let client =
            DocsRsClient::with_base_url("test", Duration::from_secs(30), &server.uri()).unwrap();
        let krate = client.fetch_rustdoc("serde", "latest").await.unwrap();
        assert_eq!(krate.crate_version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn decode_body_rejects_oversized_compressed_body() {
        // A compressed body larger than the limit is rejected before
        // decompression.
        let bytes = vec![0u8; 64];
        let err = decode_body("huge", &bytes, 32).unwrap_err();
        match err {
            DocsRsError::ResponseTooLarge { name, size, limit } => {
                assert_eq!(name, "huge");
                assert_eq!(size, 64);
                assert_eq!(limit, 32);
            }
            other => panic!("expected ResponseTooLarge, got: {other}"),
        }
    }

    #[test]
    fn decode_body_rejects_oversized_decompressed_payload() {
        // A small gzip payload that decompresses to more than the limit is
        // rejected (gzip-bomb protection).
        let original = vec![b'a'; 4096];
        let compressed = gzip_compress(&original);
        // The compressed body is under the limit, but the decompressed output
        // is not.
        assert!((compressed.len() as u64) < 1024);
        let err = decode_body("bomb", &compressed, 1024).unwrap_err();
        match err {
            DocsRsError::ResponseTooLarge { name, size, limit } => {
                assert_eq!(name, "bomb");
                assert!(size > 1024);
                assert_eq!(limit, 1024);
            }
            other => panic!("expected ResponseTooLarge, got: {other}"),
        }
    }

    #[test]
    fn decode_body_accepts_within_limit() {
        let original = synthetic_crate_json();
        let compressed = gzip_compress(&original);
        let decoded = decode_body("ok", &compressed, MAX_RESPONSE_BYTES).unwrap();
        assert_eq!(decoded, original);

        // Plain (non-gzip) bodies pass through unchanged.
        let decoded_plain = decode_body("ok", &original, MAX_RESPONSE_BYTES).unwrap();
        assert_eq!(decoded_plain, original);
    }

    #[tokio::test]
    async fn fetch_rustdoc_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crate/nonexistent/latest/json.gz"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client =
            DocsRsClient::with_base_url("test", Duration::from_secs(30), &server.uri()).unwrap();
        let err = client
            .fetch_rustdoc("nonexistent", "latest")
            .await
            .unwrap_err();
        assert!(matches!(err, DocsRsError::NotFound { .. }));
    }

    #[tokio::test]
    async fn fetch_rustdoc_not_available() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crate/oldcrate/0.1.0/json.gz"))
            .respond_with(ResponseTemplate::new(406))
            .mount(&server)
            .await;

        let client =
            DocsRsClient::with_base_url("test", Duration::from_secs(30), &server.uri()).unwrap();
        let err = client.fetch_rustdoc("oldcrate", "0.1.0").await.unwrap_err();
        assert!(matches!(err, DocsRsError::DocsNotAvailable { .. }));
    }

    #[tokio::test]
    async fn fetch_rustdoc_parse_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crate/bad/latest/json.gz"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("not json")
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let client =
            DocsRsClient::with_base_url("test", Duration::from_secs(30), &server.uri()).unwrap();
        let err = client.fetch_rustdoc("bad", "latest").await.unwrap_err();
        assert!(matches!(err, DocsRsError::Parse { .. }));
    }

    #[tokio::test]
    async fn fetch_rustdoc_format_mismatch_warning() {
        // Serve JSON with a different (but structurally compatible) format version.
        // Parsing should still succeed, but a warning is logged.
        let mismatched_version = rustdoc_types::FORMAT_VERSION + 1;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crate/testcrate/latest/json.gz"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(synthetic_crate_json_with_version(mismatched_version))
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let client =
            DocsRsClient::with_base_url("test", Duration::from_secs(30), &server.uri()).unwrap();
        // Should succeed despite version mismatch (structure is compatible)
        let krate = client.fetch_rustdoc("testcrate", "latest").await.unwrap();
        assert_eq!(krate.crate_version.as_deref(), Some("1.0.0"));
    }

    #[tokio::test]
    async fn fetch_rustdoc_format_mismatch_error() {
        // Serve JSON with mismatched format version AND invalid structure.
        // The error should be FormatMismatch with version info.
        let mismatched_version = rustdoc_types::FORMAT_VERSION + 5;
        let json = serde_json::json!({
            "root": 0,
            "format_version": mismatched_version,
            "invalid_field": true
        });
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crate/badcrate/latest/json.gz"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(serde_json::to_vec(&json).unwrap())
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let client =
            DocsRsClient::with_base_url("test", Duration::from_secs(30), &server.uri()).unwrap();
        let err = client
            .fetch_rustdoc("badcrate", "latest")
            .await
            .unwrap_err();
        match &err {
            DocsRsError::FormatMismatch {
                name,
                expected,
                actual,
                ..
            } => {
                assert_eq!(name, "badcrate");
                assert_eq!(*expected, rustdoc_types::FORMAT_VERSION);
                assert_eq!(*actual, mismatched_version);
            }
            other => panic!("expected FormatMismatch, got: {other}"),
        }
        let msg = err.to_string();
        assert!(msg.contains("format v"));
        assert!(msg.contains("consider updating the rustdoc-types dependency"));
    }
}
