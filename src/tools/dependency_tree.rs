//! Recursive dependency tree tool

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use tower_mcp::{
    CallToolResult, ResultExt, Tool, ToolBuilder,
    extract::{Json, State},
};

use crate::client::types::Dependency;
use crate::state::AppState;

/// Input for getting a dependency tree
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DependencyTreeInput {
    /// Crate name
    name: String,
    /// Version (default: latest)
    version: Option<String>,
    /// Maximum depth to recurse (default: 3, max: 5)
    max_depth: Option<u32>,
}

/// A node in the dependency tree used during BFS traversal.
struct TreeNode {
    name: String,
    version: String,
    deps: Vec<TreeChild>,
}

/// A child reference in the formatted tree.
struct TreeChild {
    name: String,
    req: String,
    optional: bool,
    /// None = not yet expanded (depth exceeded), Some = index into nodes vec
    node_idx: Option<usize>,
    seen: bool,
    circular: bool,
}

/// Cached info about a resolved crate.
struct ResolvedCrate {
    version: String,
    deps: Vec<Dependency>,
}

/// Format the tree output recursively.
fn format_tree(
    nodes: &[TreeNode],
    node_idx: usize,
    prefix: &str,
    is_last: bool,
    is_root: bool,
    output: &mut String,
) {
    let node = &nodes[node_idx];

    if is_root {
        output.push_str(&format!("{} v{}\n", node.name, node.version));
    }

    for (i, child) in node.deps.iter().enumerate() {
        let child_is_last = i == node.deps.len() - 1;
        let connector = "+-- ";
        let child_prefix = if is_root {
            "".to_string()
        } else if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}|   ", prefix)
        };

        let suffix = if child.circular {
            " (circular)"
        } else if child.seen {
            " (seen)"
        } else {
            ""
        };
        let opt = if child.optional { " (optional)" } else { "" };

        output.push_str(&format!(
            "{}{}{} {}{}{}\n",
            child_prefix, connector, child.name, child.req, opt, suffix
        ));

        // Recurse into children that have been expanded
        if let Some(idx) = child.node_idx
            && !child.seen
            && !child.circular
        {
            format_tree(nodes, idx, &child_prefix, child_is_last, false, output);
        }
    }
}

pub fn build(state: Arc<AppState>) -> Tool {
    ToolBuilder::new("get_dependency_tree")
        .description(
            "Get the full transitive dependency tree for a crate, recursively resolving \
             dependencies to a configurable depth. Shows the complete dependency footprint \
             with version requirements and deduplication markers.",
        )
        .read_only()
        .idempotent()
        .icon("https://crates.io/assets/cargo.png")
        .extractor_handler(
            state,
            |State(state): State<Arc<AppState>>,
             Json(input): Json<DependencyTreeInput>| async move {
                let max_depth = input.max_depth.unwrap_or(3).min(5);

                // Resolve root crate version
                let crate_response = state
                    .client
                    .get_crate(&input.name)
                    .await
                    .tool_context("Crates.io API error")?;

                let root_version = input
                    .version
                    .as_deref()
                    .unwrap_or(&crate_response.crate_data.max_version)
                    .to_string();

                // Cache: crate_name -> ResolvedCrate
                let mut cache: HashMap<String, ResolvedCrate> = HashMap::new();

                // Fetch root deps
                let root_deps = state
                    .client
                    .crate_dependencies(&input.name, &root_version)
                    .await
                    .tool_context("Crates.io API error")?;

                let mut api_calls: u32 = 2; // get_crate + crate_dependencies for root

                cache.insert(
                    input.name.clone(),
                    ResolvedCrate {
                        version: root_version.clone(),
                        deps: root_deps,
                    },
                );

                // BFS queue: (crate_name, depth)
                // We process each crate's normal deps and resolve their versions
                let mut queue: VecDeque<(String, u32)> = VecDeque::new();
                queue.push_back((input.name.clone(), 0));

                // Track which crates we've queued to avoid re-processing
                let mut queued: HashSet<String> = HashSet::new();
                queued.insert(input.name.clone());

                while let Some((crate_name, depth)) = queue.pop_front() {
                    if depth >= max_depth {
                        continue;
                    }

                    let deps = {
                        let resolved = cache.get(&crate_name).expect("crate should be cached");
                        resolved
                            .deps
                            .iter()
                            .filter(|d| d.kind == "normal")
                            .cloned()
                            .collect::<Vec<_>>()
                    };

                    for dep in &deps {
                        if queued.contains(&dep.crate_id) {
                            continue;
                        }
                        queued.insert(dep.crate_id.clone());

                        // Resolve the dep's actual version via get_crate
                        let dep_crate = match state.client.get_crate(&dep.crate_id).await {
                            Ok(c) => c,
                            Err(_) => continue, // skip unresolvable deps
                        };
                        api_calls += 1;

                        let dep_version = dep_crate.crate_data.max_version.clone();

                        // Fetch the dep's own dependencies
                        let dep_deps: Vec<Dependency> = state
                            .client
                            .crate_dependencies(&dep.crate_id, &dep_version)
                            .await
                            .unwrap_or_default();
                        api_calls += 1;

                        cache.insert(
                            dep.crate_id.clone(),
                            ResolvedCrate {
                                version: dep_version,
                                deps: dep_deps,
                            },
                        );

                        queue.push_back((dep.crate_id.clone(), depth + 1));
                    }
                }

                // Build tree structure from cache
                // We build nodes bottom-up via a recursive function
                let mut nodes: Vec<TreeNode> = Vec::new();
                let mut node_map: HashMap<String, usize> = HashMap::new();
                let mut building: HashSet<String> = HashSet::new();

                fn build_node(
                    crate_name: &str,
                    cache: &HashMap<String, ResolvedCrate>,
                    nodes: &mut Vec<TreeNode>,
                    node_map: &mut HashMap<String, usize>,
                    building: &mut HashSet<String>,
                    depth: u32,
                    max_depth: u32,
                ) -> usize {
                    if let Some(&idx) = node_map.get(crate_name) {
                        return idx;
                    }

                    let resolved = match cache.get(crate_name) {
                        Some(r) => r,
                        None => {
                            // Crate not in cache (couldn't resolve)
                            let idx = nodes.len();
                            nodes.push(TreeNode {
                                name: crate_name.to_string(),
                                version: "?".to_string(),
                                deps: Vec::new(),
                            });
                            node_map.insert(crate_name.to_string(), idx);
                            return idx;
                        }
                    };

                    // Mark as being built (circular detection)
                    building.insert(crate_name.to_string());

                    let normal_deps: Vec<Dependency> = resolved
                        .deps
                        .iter()
                        .filter(|d| d.kind == "normal")
                        .cloned()
                        .collect();

                    let mut children = Vec::new();

                    for dep in &normal_deps {
                        if building.contains(&dep.crate_id) {
                            // Circular dependency
                            children.push(TreeChild {
                                name: dep.crate_id.clone(),
                                req: dep.req.clone(),
                                optional: dep.optional,
                                node_idx: None,
                                seen: false,
                                circular: true,
                            });
                        } else if node_map.contains_key(&dep.crate_id) {
                            // Already seen at a different point in the tree
                            children.push(TreeChild {
                                name: dep.crate_id.clone(),
                                req: dep.req.clone(),
                                optional: dep.optional,
                                node_idx: Some(node_map[&dep.crate_id]),
                                seen: true,
                                circular: false,
                            });
                        } else if depth + 1 > max_depth || !cache.contains_key(&dep.crate_id) {
                            // Depth exceeded or not resolved
                            children.push(TreeChild {
                                name: dep.crate_id.clone(),
                                req: dep.req.clone(),
                                optional: dep.optional,
                                node_idx: None,
                                seen: false,
                                circular: false,
                            });
                        } else {
                            // Recurse
                            let child_idx = build_node(
                                &dep.crate_id,
                                cache,
                                nodes,
                                node_map,
                                building,
                                depth + 1,
                                max_depth,
                            );
                            children.push(TreeChild {
                                name: dep.crate_id.clone(),
                                req: dep.req.clone(),
                                optional: dep.optional,
                                node_idx: Some(child_idx),
                                seen: false,
                                circular: false,
                            });
                        }
                    }

                    building.remove(crate_name);

                    let idx = nodes.len();
                    nodes.push(TreeNode {
                        name: crate_name.to_string(),
                        version: resolved.version.clone(),
                        deps: children,
                    });
                    node_map.insert(crate_name.to_string(), idx);
                    idx
                }

                let root_idx = build_node(
                    &input.name,
                    &cache,
                    &mut nodes,
                    &mut node_map,
                    &mut building,
                    0,
                    max_depth,
                );

                // Format tree output
                let mut output =
                    format!("# Dependency Tree: {} v{}\n\n", input.name, root_version);

                format_tree(&nodes, root_idx, "", true, true, &mut output);

                // Count stats
                let direct_deps = cache
                    .get(&input.name)
                    .map(|r| r.deps.iter().filter(|d| d.kind == "normal").count())
                    .unwrap_or(0);
                let unique_crates = cache.len() - 1; // exclude root

                // Calculate max depth reached
                fn calc_depth(
                    nodes: &[TreeNode],
                    idx: usize,
                    seen: &mut HashSet<usize>,
                ) -> u32 {
                    if seen.contains(&idx) {
                        return 0;
                    }
                    seen.insert(idx);
                    let node = &nodes[idx];
                    let mut max = 0;
                    for child in &node.deps {
                        if let Some(child_idx) = child.node_idx
                            && !child.seen
                            && !child.circular
                        {
                            let d = calc_depth(nodes, child_idx, seen);
                            max = max.max(d);
                        }
                    }
                    if node.deps.is_empty() { 0 } else { max + 1 }
                }

                let mut depth_seen = HashSet::new();
                let tree_depth = calc_depth(&nodes, root_idx, &mut depth_seen);

                output.push_str(&format!(
                    "\n## Summary\n\n\
                     - **Direct dependencies**: {}\n\
                     - **Total unique crates in tree**: {}\n\
                     - **Tree depth**: {}\n\
                     - **API calls made**: {}\n",
                    direct_deps, unique_crates, tree_depth, api_calls
                ));

                Ok(CallToolResult::text(output))
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
            client: CratesIoClient::with_base_url("test", Duration::from_millis(0), base_url)
                .unwrap(),
            docsrs_client: DocsRsClient::with_base_url("test", base_url).unwrap(),
            osv_client: OsvClient::new("test").unwrap(),
            docs_cache: DocsCache::new(10, Duration::from_secs(3600)),
            recent_searches: RwLock::new(Vec::new()),
        })
    }

    #[tokio::test]
    async fn dependency_tree_basic() {
        let server = MockServer::start().await;

        // Root crate: my-crate v1.0.0 depends on dep-a
        Mock::given(method("GET"))
            .and(path("/crates/my-crate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "my-crate",
                    "max_version": "1.0.0",
                    "description": "Test crate",
                    "downloads": 100,
                    "created_at": "2026-01-01T00:00:00.000000Z",
                    "updated_at": "2026-01-01T00:00:00.000000Z"
                },
                "versions": [{"num": "1.0.0", "yanked": false, "created_at": "2026-01-01T00:00:00.000000Z", "downloads": 100}]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/my-crate/1.0.0/dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": [
                    {"crate_id": "dep-a", "req": "^2.0", "kind": "normal", "optional": false, "version_id": 1}
                ]
            })))
            .mount(&server)
            .await;

        // dep-a v2.0.0 depends on dep-b
        Mock::given(method("GET"))
            .and(path("/crates/dep-a"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "dep-a",
                    "max_version": "2.0.0",
                    "description": "Dep A",
                    "downloads": 50,
                    "created_at": "2026-01-01T00:00:00.000000Z",
                    "updated_at": "2026-01-01T00:00:00.000000Z"
                },
                "versions": [{"num": "2.0.0", "yanked": false, "created_at": "2026-01-01T00:00:00.000000Z", "downloads": 50}]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/dep-a/2.0.0/dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": [
                    {"crate_id": "dep-b", "req": "^1.0", "kind": "normal", "optional": false, "version_id": 2}
                ]
            })))
            .mount(&server)
            .await;

        // dep-b v1.0.0 has no deps
        Mock::given(method("GET"))
            .and(path("/crates/dep-b"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "dep-b",
                    "max_version": "1.0.0",
                    "description": "Dep B",
                    "downloads": 30,
                    "created_at": "2026-01-01T00:00:00.000000Z",
                    "updated_at": "2026-01-01T00:00:00.000000Z"
                },
                "versions": [{"num": "1.0.0", "yanked": false, "created_at": "2026-01-01T00:00:00.000000Z", "downloads": 30}]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/dep-b/1.0.0/dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": []
            })))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "my-crate"})).await;

        let text = result.all_text();
        assert!(text.contains("Dependency Tree: my-crate v1.0.0"));
        assert!(text.contains("dep-a"));
        assert!(text.contains("dep-b"));
        assert!(text.contains("Direct dependencies"));
        assert!(text.contains("Total unique crates in tree"));
    }

    #[tokio::test]
    async fn dependency_tree_with_seen_deps() {
        let server = MockServer::start().await;

        // Root depends on dep-a and dep-b, both depend on dep-shared
        Mock::given(method("GET"))
            .and(path("/crates/root"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "root",
                    "max_version": "1.0.0",
                    "description": "Root",
                    "downloads": 100,
                    "created_at": "2026-01-01T00:00:00.000000Z",
                    "updated_at": "2026-01-01T00:00:00.000000Z"
                },
                "versions": [{"num": "1.0.0", "yanked": false, "created_at": "2026-01-01T00:00:00.000000Z", "downloads": 100}]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/root/1.0.0/dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": [
                    {"crate_id": "dep-a", "req": "^1", "kind": "normal", "optional": false, "version_id": 1},
                    {"crate_id": "dep-b", "req": "^1", "kind": "normal", "optional": false, "version_id": 2}
                ]
            })))
            .mount(&server)
            .await;

        // dep-a depends on dep-shared
        Mock::given(method("GET"))
            .and(path("/crates/dep-a"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "dep-a",
                    "max_version": "1.0.0",
                    "description": "Dep A",
                    "downloads": 50,
                    "created_at": "2026-01-01T00:00:00.000000Z",
                    "updated_at": "2026-01-01T00:00:00.000000Z"
                },
                "versions": [{"num": "1.0.0", "yanked": false, "created_at": "2026-01-01T00:00:00.000000Z", "downloads": 50}]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/dep-a/1.0.0/dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": [
                    {"crate_id": "dep-shared", "req": "^1", "kind": "normal", "optional": false, "version_id": 3}
                ]
            })))
            .mount(&server)
            .await;

        // dep-b depends on dep-shared
        Mock::given(method("GET"))
            .and(path("/crates/dep-b"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "dep-b",
                    "max_version": "1.0.0",
                    "description": "Dep B",
                    "downloads": 30,
                    "created_at": "2026-01-01T00:00:00.000000Z",
                    "updated_at": "2026-01-01T00:00:00.000000Z"
                },
                "versions": [{"num": "1.0.0", "yanked": false, "created_at": "2026-01-01T00:00:00.000000Z", "downloads": 30}]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/dep-b/1.0.0/dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": [
                    {"crate_id": "dep-shared", "req": "^1", "kind": "normal", "optional": false, "version_id": 3}
                ]
            })))
            .mount(&server)
            .await;

        // dep-shared has no deps
        Mock::given(method("GET"))
            .and(path("/crates/dep-shared"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "dep-shared",
                    "max_version": "1.0.0",
                    "description": "Shared",
                    "downloads": 200,
                    "created_at": "2026-01-01T00:00:00.000000Z",
                    "updated_at": "2026-01-01T00:00:00.000000Z"
                },
                "versions": [{"num": "1.0.0", "yanked": false, "created_at": "2026-01-01T00:00:00.000000Z", "downloads": 200}]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/dep-shared/1.0.0/dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": []
            })))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool.call(serde_json::json!({"name": "root"})).await;

        let text = result.all_text();
        assert!(text.contains("dep-shared"));
        // dep-shared should appear as (seen) in one of the branches
        assert!(text.contains("(seen)"));
    }

    #[tokio::test]
    async fn dependency_tree_max_depth_zero() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/crates/my-crate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "crate": {
                    "name": "my-crate",
                    "max_version": "1.0.0",
                    "description": "Test",
                    "downloads": 100,
                    "created_at": "2026-01-01T00:00:00.000000Z",
                    "updated_at": "2026-01-01T00:00:00.000000Z"
                },
                "versions": [{"num": "1.0.0", "yanked": false, "created_at": "2026-01-01T00:00:00.000000Z", "downloads": 100}]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/crates/my-crate/1.0.0/dependencies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "dependencies": [
                    {"crate_id": "dep-a", "req": "^1", "kind": "normal", "optional": false, "version_id": 1}
                ]
            })))
            .mount(&server)
            .await;

        let state = test_state(&server.uri());
        let tool = super::build(state);
        let result = tool
            .call(serde_json::json!({"name": "my-crate", "max_depth": 0}))
            .await;

        let text = result.all_text();
        // Should show root and direct deps but not recurse further
        assert!(text.contains("Dependency Tree: my-crate v1.0.0"));
        assert!(text.contains("dep-a"));
    }
}
