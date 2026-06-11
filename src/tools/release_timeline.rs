//! Get release timeline tool

use std::collections::HashSet;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::state::AppState;

/// Input for the release timeline tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReleaseTimelineInput {
    /// Crate name
    name: String,
    /// How many recent versions to compare (default 5, clamped to [2, 10])
    #[serde(default)]
    versions: Option<usize>,
    /// Include yanked versions in the timeline (default true)
    #[serde(default)]
    include_yanked: Option<bool>,
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_release_timeline")
        .title("Get Release Timeline")
        .description(
            "Show a version-over-version registry-metadata diff for a crate: feature flag \
             changes, MSRV bumps, license changes, yanked status, and release cadence. \
             Uses a single crates.io API call on the happy path.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>,
             Json(input): Json<ReleaseTimelineInput>| async move {
                let name = input.name.trim().to_owned();
                let limit = input.versions.unwrap_or(5).clamp(2, 10);
                let include_yanked = input.include_yanked.unwrap_or(true);

                let response = state
                    .client
                    .get_crate(&name)
                    .await
                    .tool_context("Crates.io API error")?;

                // crates.io returns versions newest-first
                let window: Vec<_> = response
                    .versions
                    .into_iter()
                    .filter(|v| include_yanked || !v.yanked)
                    .take(limit)
                    .collect();

                if window.is_empty() {
                    return Ok(CallToolResult::text(format!(
                        "No versions found for `{name}`."
                    )));
                }

                // If all embedded versions have empty features, fall back to per-version calls
                let needs_fallback = window.iter().all(|v| v.features.is_empty());
                let mut api_calls = 1usize;

                let versions = if needs_fallback {
                    let mut enriched = Vec::with_capacity(window.len());
                    for v in window {
                        match state.client.crate_version(&name, &v.num).await {
                            Ok(detail) => {
                                api_calls += 1;
                                enriched.push(detail);
                            }
                            Err(_) => {
                                enriched.push(v);
                            }
                        }
                    }
                    enriched
                } else {
                    window
                };

                // Single version: nothing to compare
                if versions.len() == 1 {
                    let v = &versions[0];
                    let yanked_tag = if v.yanked { " [YANKED]" } else { "" };
                    let mut out = format!("# Release Timeline: {name} (1 version)\n\n");
                    out.push_str(&format!(
                        "## v{}{} -- {}\n",
                        v.num,
                        yanked_tag,
                        v.created_at.date_naive()
                    ));
                    if let Some(msrv) = &v.rust_version {
                        out.push_str(&format!("- MSRV: {msrv}\n"));
                    }
                    if let Some(lic) = &v.license {
                        out.push_str(&format!("- License: {lic}\n"));
                    }
                    if !v.features.is_empty() {
                        let mut feats: Vec<_> = v.features.keys().map(|s| s.as_str()).collect();
                        feats.sort();
                        out.push_str(&format!("- Features: {}\n", feats.join(", ")));
                    }
                    out.push('\n');
                    out.push_str("Only one version published -- nothing to compare.\n\n");
                    out.push_str("## Summary\n");
                    out.push_str("- Versions compared: 1\n");
                    out.push_str(&format!("- API calls made: {api_calls}\n"));
                    return Ok(CallToolResult::text(out));
                }

                let total = versions.len();
                let mut out = format!("# Release Timeline: {name} (last {total} versions)\n\n");

                let mut yanked_count = 0usize;
                let mut msrv_change_count = 0usize;
                let mut feature_change_count = 0usize;
                let mut cadence_days: Vec<i64> = Vec::new();

                for i in 0..total {
                    let cur = &versions[i];
                    // versions[i+1] is chronologically older (previous release)
                    let prev = versions.get(i + 1);

                    if cur.yanked {
                        yanked_count += 1;
                    }

                    let yanked_tag = if cur.yanked { "  [YANKED]" } else { "" };

                    let cadence_str = if let Some(p) = prev {
                        let delta = (cur.created_at - p.created_at).num_days().abs();
                        cadence_days.push(delta);
                        format!("  (+{delta} days)")
                    } else {
                        "  (oldest in window)".to_string()
                    };

                    out.push_str(&format!(
                        "## v{} -- {}{}{}\n",
                        cur.num,
                        cur.created_at.date_naive(),
                        cadence_str,
                        yanked_tag
                    ));

                    // MSRV: only diff when both sides are Some and differ; never a false change
                    let prev_msrv = prev.and_then(|p| p.rust_version.as_deref());
                    match (prev.is_some(), cur.rust_version.as_deref()) {
                        (false, Some(msrv)) => {
                            out.push_str(&format!("- MSRV: {msrv}\n"));
                        }
                        (true, Some(cur_msrv)) => match prev_msrv {
                            Some(old) if old != cur_msrv => {
                                out.push_str(&format!("- MSRV: {old} → {cur_msrv}  (changed)\n"));
                                msrv_change_count += 1;
                            }
                            Some(_) => {
                                out.push_str(&format!("- MSRV: {cur_msrv} (unchanged)\n"));
                            }
                            None => {
                                // prev had no MSRV info -- don't claim a change
                                out.push_str(&format!("- MSRV: {cur_msrv}\n"));
                            }
                        },
                        _ => {} // no MSRV info for cur, skip
                    }

                    // License: show only on change (when both sides are Some and differ)
                    if let Some(p) = prev
                        && let (Some(old_lic), Some(new_lic)) =
                            (p.license.as_deref(), cur.license.as_deref())
                        && old_lic != new_lic
                    {
                        out.push_str(&format!(
                            "- License: {old_lic} → {new_lic}  (changed)\n"
                        ));
                    }

                    // Features: set-diff on feature names (keys of the HashMap)
                    if let Some(p) = prev {
                        let cur_feats: HashSet<&String> = cur.features.keys().collect();
                        let prev_feats: HashSet<&String> = p.features.keys().collect();
                        let mut added: Vec<_> = cur_feats.difference(&prev_feats).collect();
                        let mut removed: Vec<_> = prev_feats.difference(&cur_feats).collect();

                        if needs_fallback && cur.features.is_empty() && p.features.is_empty() {
                            out.push_str("- Features: data unavailable\n");
                        } else if added.is_empty() && removed.is_empty() {
                            out.push_str("- Features: no change\n");
                        } else {
                            added.sort();
                            removed.sort();
                            let mut parts: Vec<String> = Vec::new();
                            for f in &added {
                                parts.push(format!("+`{f}`"));
                            }
                            for f in &removed {
                                parts.push(format!("-`{f}`"));
                            }
                            out.push_str(&format!("- Features: {}\n", parts.join(", ")));
                            feature_change_count += 1;
                        }
                    } else {
                        // Oldest in window: list current features
                        if cur.features.is_empty() {
                            if needs_fallback {
                                out.push_str("- Features: data unavailable\n");
                            }
                        } else {
                            let mut feats: Vec<_> =
                                cur.features.keys().map(|s| s.as_str()).collect();
                            feats.sort();
                            out.push_str(&format!("- Features: {}\n", feats.join(", ")));
                        }
                    }

                    out.push('\n');
                }

                let oldest = versions.last().unwrap();
                let newest = versions.first().unwrap();
                let span_start = oldest.created_at.date_naive();
                let span_end = newest.created_at.date_naive();
                let avg_cadence = if cadence_days.is_empty() {
                    "n/a".to_string()
                } else {
                    let sum: i64 = cadence_days.iter().sum();
                    format!("~{:.1} days", sum as f64 / cadence_days.len() as f64)
                };

                out.push_str("## Summary\n");
                out.push_str(&format!(
                    "- Versions compared: {total} ({yanked_count} yanked)\n"
                ));
                out.push_str(&format!("- Span: {span_start} → {span_end}\n"));
                out.push_str(&format!(
                    "- Average cadence: {avg_cadence} between releases\n"
                ));
                out.push_str(&format!("- MSRV changes: {msrv_change_count}\n"));
                out.push_str(&format!(
                    "- Feature changes: {feature_change_count} versions added/removed features\n"
                ));
                out.push_str(&format!("- API calls made: {api_calls}\n"));

                Ok(CallToolResult::text(out))
            },
        )
        .build()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::sync::RwLock;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::client::CratesIoClient;
    use crate::client::docsrs::DocsRsClient;
    use crate::client::osv::OsvClient;
    use crate::docs::cache::DocsCache;
    use crate::state::AppState;

    fn test_state(base_url: &str) -> Arc<AppState> {
        Arc::new(AppState {
            client: CratesIoClient::with_base_url(
                "test",
                Duration::from_millis(0),
                Duration::from_secs(30),
                base_url,
            )
            .unwrap(),
            docsrs_client: DocsRsClient::with_base_url("test", Duration::from_secs(30), base_url)
                .unwrap(),
            osv_client: OsvClient::with_base_url(
                "test",
                Duration::from_secs(30),
                "http://localhost:1",
            )
            .unwrap(),
            docs_cache: DocsCache::new(10, Duration::from_secs(3600)),
            recent_searches: RwLock::new(Vec::new()),
        })
    }

    #[tokio::test]
    async fn release_timeline_multi_version() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/my-crate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "my-crate",
                    "max_version": "1.2.0",
                    "downloads": 1000,
                    "created_at": "2024-01-01T00:00:00.000000Z",
                    "updated_at": "2024-09-09T00:00:00.000000Z"
                },
                "versions": [
                    {
                        "num": "1.2.0",
                        "yanked": false,
                        "created_at": "2024-09-09T00:00:00.000000Z",
                        "downloads": 500,
                        "license": "MIT",
                        "rust_version": "1.65",
                        "features": {"std": [], "alloc": []}
                    },
                    {
                        "num": "1.1.0",
                        "yanked": true,
                        "created_at": "2024-08-27T00:00:00.000000Z",
                        "downloads": 300,
                        "license": "MIT",
                        "rust_version": "1.60",
                        "features": {"std": []}
                    },
                    {
                        "num": "1.0.0",
                        "yanked": false,
                        "created_at": "2024-01-01T00:00:00.000000Z",
                        "downloads": 200,
                        "license": "MIT",
                        "rust_version": "1.60",
                        "features": {"std": []}
                    }
                ]
            })))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"name": "my-crate", "versions": 3}))
            .await;

        assert!(!result.is_error, "unexpected error: {}", result.all_text());
        let text = result.all_text();

        // MSRV change from 1.60 to 1.65
        assert!(
            text.contains("1.60") && text.contains("1.65") && text.contains("changed"),
            "expected MSRV change line, got: {text}"
        );

        // Feature +`alloc` added in v1.2.0
        assert!(
            text.contains("+`alloc`"),
            "expected +`alloc` feature addition, got: {text}"
        );

        // v1.1.0 is yanked
        assert!(
            text.contains("[YANKED]"),
            "expected [YANKED] tag, got: {text}"
        );

        // Summary block present with cadence info
        assert!(
            text.contains("## Summary"),
            "expected Summary section, got: {text}"
        );
        assert!(
            text.contains("API calls made: 1"),
            "expected 1 API call on happy path, got: {text}"
        );
    }

    #[tokio::test]
    async fn release_timeline_features_fallback() {
        let server = MockServer::start().await;

        // Embedded features are empty -- triggers fallback
        Mock::given(method("GET"))
            .and(path("/crates/fallback-crate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "fallback-crate",
                    "max_version": "1.0.0",
                    "downloads": 100,
                    "created_at": "2024-01-01T00:00:00.000000Z",
                    "updated_at": "2024-06-01T00:00:00.000000Z"
                },
                "versions": [
                    {
                        "num": "1.0.0",
                        "yanked": false,
                        "created_at": "2024-06-01T00:00:00.000000Z",
                        "downloads": 80,
                        "license": "MIT",
                        "rust_version": "1.70",
                        "features": {}
                    },
                    {
                        "num": "0.9.0",
                        "yanked": false,
                        "created_at": "2024-01-01T00:00:00.000000Z",
                        "downloads": 20,
                        "license": "MIT",
                        "rust_version": "1.70",
                        "features": {}
                    }
                ]
            })))
            .mount(&server)
            .await;

        // Per-version fallback calls supply features
        Mock::given(method("GET"))
            .and(path("/crates/fallback-crate/1.0.0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "version": {
                    "num": "1.0.0",
                    "yanked": false,
                    "created_at": "2024-06-01T00:00:00.000000Z",
                    "downloads": 80,
                    "license": "MIT",
                    "rust_version": "1.70",
                    "features": {"default": [], "extra": []}
                }
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/fallback-crate/0.9.0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "version": {
                    "num": "0.9.0",
                    "yanked": false,
                    "created_at": "2024-01-01T00:00:00.000000Z",
                    "downloads": 20,
                    "license": "MIT",
                    "rust_version": "1.70",
                    "features": {"default": []}
                }
            })))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"name": "fallback-crate", "versions": 2}))
            .await;

        assert!(!result.is_error, "unexpected error: {}", result.all_text());
        let text = result.all_text();

        // Fallback added 2 per-version calls: total = 3
        assert!(
            text.contains("API calls made: 3"),
            "expected 3 API calls after fallback, got: {text}"
        );

        // Feature `extra` was added in 1.0.0 vs 0.9.0
        assert!(
            text.contains("+`extra`"),
            "expected +`extra` from fallback feature diff, got: {text}"
        );
    }

    #[tokio::test]
    async fn release_timeline_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/no-such-crate"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"name": "no-such-crate"}))
            .await;

        assert!(result.is_error, "expected error for missing crate");
    }
}
