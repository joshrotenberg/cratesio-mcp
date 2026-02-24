use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, ValueEnum};
use cratesio_mcp::{prompts, resources, state::AppState, tools};
use tower::ServiceBuilder;
use tower::timeout::TimeoutLayer;
use tower_mcp::protocol::{
    CallToolParams, CompleteParams, CompleteResult, Completion, CompletionReference, McpRequest,
};
use tower_mcp::router::{RouterRequest, RouterResponse};
use tower_mcp::{HttpTransport, McpRouter, McpTracingLayer, StdioTransport};
use tower_resilience::bulkhead::BulkheadLayer;
use tower_resilience::cache::SharedCacheLayer;
use tower_resilience::ratelimiter::RateLimiterLayer;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Transport {
    Stdio,
    Http,
}

#[derive(Parser, Debug)]
#[command(name = "cratesio-mcp")]
#[command(about = "MCP server for querying crates.io", long_about = None)]
struct Args {
    /// Transport to use
    #[arg(short, long, default_value = "stdio")]
    transport: Transport,

    /// Maximum concurrent requests (concurrency limit)
    #[arg(long, default_value = "10")]
    max_concurrent: usize,

    /// Rate limit interval between crates.io API calls (in milliseconds)
    #[arg(long, default_value = "1000")]
    rate_limit_ms: u64,

    /// Log level
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// HTTP host to bind to (use 0.0.0.0 for public access)
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// HTTP port to bind to
    #[arg(short, long, default_value = "3000")]
    port: u16,

    /// Request timeout in seconds (for HTTP transport)
    #[arg(long, default_value = "30")]
    request_timeout_secs: u64,

    /// Minimal mode - only register tools (no prompts, resources, or completions).
    /// Use this to work around Claude Code MCP tool discovery issues.
    /// See: https://github.com/anthropics/claude-code/issues/2682
    #[arg(long, default_value = "false")]
    minimal: bool,

    /// Enable response caching for tool calls (HTTP transport only)
    #[arg(long, default_value = "true")]
    cache_enabled: bool,

    /// Cache TTL in seconds (how long cached responses are valid)
    #[arg(long, default_value = "300")]
    cache_ttl_secs: u64,

    /// Maximum number of cached responses
    #[arg(long, default_value = "200")]
    cache_max_size: usize,

    /// Maximum number of cached docs.rs rustdoc JSON entries
    #[arg(long, default_value = "10")]
    docs_cache_max_entries: usize,

    /// TTL for cached docs.rs rustdoc JSON entries (in seconds)
    #[arg(long, default_value = "3600")]
    docs_cache_ttl_secs: u64,
}

#[tokio::main]
async fn main() -> Result<(), tower_mcp::BoxError> {
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(format!("cratesio_mcp={}", args.log_level).parse()?)
                .add_directive(format!("tower_mcp={}", args.log_level).parse()?),
        )
        .with_writer(std::io::stderr)
        .init();

    tracing::info!(
        transport = ?args.transport,
        max_concurrent = args.max_concurrent,
        rate_limit_ms = args.rate_limit_ms,
        "Starting cratesio-mcp server"
    );

    // Create shared state with rate limiting for crates.io API
    let rate_limit = Duration::from_millis(args.rate_limit_ms);
    let docs_cache_ttl = Duration::from_secs(args.docs_cache_ttl_secs);
    let state = Arc::new(
        AppState::new(rate_limit, args.docs_cache_max_entries, docs_cache_ttl)
            .map_err(|e| format!("Failed to create state: {}", e))?,
    );

    // Build all tools
    let search_tool = tools::search::build(state.clone());
    let info_tool = tools::info::build(state.clone());
    let versions_tool = tools::versions::build(state.clone());
    let deps_tool = tools::dependencies::build(state.clone());
    let reverse_deps_tool = tools::reverse_deps::build(state.clone());
    let downloads_tool = tools::downloads::build(state.clone());
    let owners_tool = tools::owners::build(state.clone());
    let summary_tool = tools::summary::build(state.clone());
    let authors_tool = tools::authors::build(state.clone());
    let user_tool = tools::user::build(state.clone());
    let readme_tool = tools::readme::build(state.clone());
    let categories_tool = tools::categories::build(state.clone());
    let keywords_tool = tools::keywords::build(state.clone());
    let version_downloads_tool = tools::version_downloads::build(state.clone());
    let version_detail_tool = tools::version_detail::build(state.clone());
    let category_tool = tools::category::build(state.clone());
    let keyword_detail_tool = tools::keyword_detail::build(state.clone());
    let get_crate_docs_tool = tools::crate_docs::build(state.clone());
    let get_doc_item_tool = tools::doc_item::build(state.clone());
    let search_docs_tool = tools::search_docs::build(state.clone());
    let audit_tool = tools::audit::build(state.clone());
    let features_tool = tools::features::build(state.clone());
    let user_stats_tool = tools::user_stats::build(state.clone());
    let compare_tool = tools::compare::build(state.clone());
    let dependency_tree_tool = tools::dependency_tree::build(state.clone());
    let health_check_tool = tools::health_check::build(state.clone());

    // Create base router with tools (always registered)
    let instructions = if args.minimal {
        "MCP server for querying crates.io - the Rust package registry.\n\n\
         Available tools:\n\
         - search_crates: Find crates by name/keywords\n\
         - get_crate_info: Get detailed crate information\n\
         - get_crate_versions: Get version history\n\
         - get_crate_readme: Get README content for a crate\n\
         - get_dependencies: Get dependencies for a version\n\
         - get_reverse_dependencies: Find crates that depend on this crate\n\
         - get_downloads: Get download statistics\n\
         - get_owners: Get crate owners/maintainers\n\
         - get_summary: Get crates.io global statistics\n\
         - get_crate_authors: Get authors for a crate version\n\
         - get_user: Get a user's profile\n\
         - get_categories: Browse crates.io categories\n\
         - get_keywords: Browse crates.io keywords\n\
         - get_version_downloads: Daily download stats for a specific version\n\
         - get_crate_version: Detailed metadata for a specific version\n\
         - get_category: Details about a specific category\n\
         - get_keyword: Details about a specific keyword\n\
         - get_crate_docs: Browse crate documentation structure from docs.rs\n\
         - get_doc_item: Get full documentation for a specific item from docs.rs\n\
         - search_docs: Search for items by name within a crate's docs\n\
         - audit_dependencies: Check deps against OSV.dev vulnerability database\n\
         - get_crate_features: Get feature flags for a crate version\n\
         - get_user_stats: Get download statistics for a crates.io user\n\
         - compare_crates: Compare two or more crates side by side\n\
         - get_dependency_tree: Get full transitive dependency tree for a crate\n\
         - crate_health_check: Comprehensive health report for a crate\n\n\
         (Running in minimal mode - resources, prompts, and completions disabled)"
    } else {
        "MCP server for querying crates.io - the Rust package registry.\n\n\
         Available tools:\n\
         - search_crates: Find crates by name/keywords\n\
         - get_crate_info: Get detailed crate information\n\
         - get_crate_versions: Get version history\n\
         - get_crate_readme: Get README content for a crate\n\
         - get_dependencies: Get dependencies for a version\n\
         - get_reverse_dependencies: Find crates that depend on this crate\n\
         - get_downloads: Get download statistics\n\
         - get_owners: Get crate owners/maintainers\n\
         - get_summary: Get crates.io global statistics\n\
         - get_crate_authors: Get authors for a crate version\n\
         - get_user: Get a user's profile\n\
         - get_categories: Browse crates.io categories\n\
         - get_keywords: Browse crates.io keywords\n\
         - get_version_downloads: Daily download stats for a specific version\n\
         - get_crate_version: Detailed metadata for a specific version\n\
         - get_category: Details about a specific category\n\
         - get_keyword: Details about a specific keyword\n\
         - get_crate_docs: Browse crate documentation structure from docs.rs\n\
         - get_doc_item: Get full documentation for a specific item from docs.rs\n\
         - search_docs: Search for items by name within a crate's docs\n\
         - audit_dependencies: Check deps against OSV.dev vulnerability database\n\
         - get_crate_features: Get feature flags for a crate version\n\
         - get_user_stats: Get download statistics for a crates.io user\n\
         - compare_crates: Compare two or more crates side by side\n\
         - get_dependency_tree: Get full transitive dependency tree for a crate\n\
         - crate_health_check: Comprehensive health report for a crate\n\n\
         Resources:\n\
         - crates://{name}/info: Get crate info as a resource\n\
         - crates://{name}/readme: Get README content for a crate\n\
         - crates://{name}/docs: Get documentation structure for a crate\n\n\
         Use the prompts for guided analysis:\n\
         - analyze_crate: Comprehensive crate analysis\n\
         - compare_crates: Compare multiple crates"
    };

    let mut router = McpRouter::new()
        .server_info("cratesio-mcp", env!("CARGO_PKG_VERSION"))
        .instructions(instructions)
        .tool(search_tool)
        .tool(info_tool)
        .tool(versions_tool)
        .tool(deps_tool)
        .tool(reverse_deps_tool)
        .tool(downloads_tool)
        .tool(owners_tool)
        .tool(summary_tool)
        .tool(authors_tool)
        .tool(user_tool)
        .tool(readme_tool)
        .tool(categories_tool)
        .tool(keywords_tool)
        .tool(version_downloads_tool)
        .tool(version_detail_tool)
        .tool(category_tool)
        .tool(keyword_detail_tool)
        .tool(get_crate_docs_tool)
        .tool(get_doc_item_tool)
        .tool(search_docs_tool)
        .tool(audit_tool)
        .tool(features_tool)
        .tool(user_stats_tool)
        .tool(compare_tool)
        .tool(dependency_tree_tool)
        .tool(health_check_tool);

    // Add resources, prompts, and completions unless in minimal mode
    // Minimal mode works around Claude Code MCP tool discovery issues
    // See: https://github.com/anthropics/claude-code/issues/2682
    if !args.minimal {
        // Build resources
        let recent_searches = resources::recent_searches::build(state.clone());
        let crate_info_template = resources::crate_info::build(state.clone());
        let readme_template = resources::readme::build(state.clone());
        let docs_template = resources::docs::build(state.clone());

        // Build prompts
        let analyze_prompt = prompts::analyze::build();
        let compare_prompt = prompts::compare::build();

        // Popular crates for completion suggestions
        let popular_crates = vec![
            "serde",
            "tokio",
            "anyhow",
            "thiserror",
            "clap",
            "tracing",
            "reqwest",
            "axum",
            "tower",
            "hyper",
            "futures",
            "async-trait",
            "rand",
            "regex",
            "chrono",
            "uuid",
            "log",
            "env_logger",
            "syn",
            "quote",
            "proc-macro2",
            "bytes",
            "http",
            "tonic",
            "prost",
            "sqlx",
            "diesel",
            "actix-web",
            "rocket",
            "warp",
            "tide",
            "poem",
            "salvo",
        ];

        router = router
            .resource(recent_searches)
            .resource_template(crate_info_template)
            .resource_template(readme_template)
            .resource_template(docs_template)
            .prompt(analyze_prompt)
            .prompt(compare_prompt)
            // Completion handler for crate name suggestions
            .completion_handler(move |params: CompleteParams| {
                let popular = popular_crates.clone();
                async move {
                    let prefix = params.argument.value.to_lowercase();

                    // Filter popular crates by prefix
                    let suggestions: Vec<String> = popular
                        .iter()
                        .filter(|name| name.starts_with(&prefix))
                        .take(10)
                        .map(|name| name.to_string())
                        .collect();

                    // Log what we're completing for
                    match &params.reference {
                        CompletionReference::Prompt { name } => {
                            tracing::debug!(%name, %prefix, "Completing prompt argument");
                        }
                        CompletionReference::Resource { uri } => {
                            tracing::debug!(%uri, %prefix, "Completing resource URI");
                        }
                    }

                    Ok(CompleteResult {
                        completion: Completion {
                            values: suggestions,
                            total: None,
                            has_more: Some(false),
                        },
                        meta: None,
                    })
                }
            });

        tracing::info!("Full mode: resources, prompts, and completions enabled");
    } else {
        tracing::info!(
            "Minimal mode: only tools registered (workaround for Claude Code MCP issues)"
        );
    }

    let router = router;

    match args.transport {
        Transport::Stdio => {
            // For stdio, we serve directly without middleware since error handling
            // is more complex (would need error type conversion).
            tracing::info!("Serving over stdio");
            StdioTransport::new(router).run().await?;
        }
        Transport::Http => {
            let addr = format!("{}:{}", args.host, args.port);
            tracing::info!(
                %addr,
                cache_enabled = args.cache_enabled,
                cache_ttl_secs = args.cache_ttl_secs,
                cache_max_size = args.cache_max_size,
                "Serving over HTTP"
            );

            // Build tower middleware stack for request protection:
            //
            // 1. TimeoutLayer - Request timeout protection
            // 2. RateLimiterLayer - Limits requests per second (token bucket)
            // 3. BulkheadLayer - Limits concurrent in-flight requests
            // 4. CacheLayer - Response caching for tool calls (optional)
            //
            // These layers compose naturally with tower-mcp's Service implementation.
            // The HTTP transport's CatchError wrapper converts middleware errors
            // to JSON-RPC error responses.
            //
            // tower-resilience layers use composite error types that wrap both
            // the layer's own errors and the inner service error, making them
            // compatible with tower-mcp's Infallible error type.
            //
            // Note: CircuitBreakerLayer could be added for downstream service failures
            // (e.g., crates.io API), but McpRouter returns Infallible so the breaker
            // would need a custom failure classifier to inspect response content.
            let rate_limiter = RateLimiterLayer::builder()
                .limit_for_period(10) // 10 requests per second
                .refresh_period(Duration::from_secs(1))
                .timeout_duration(Duration::from_millis(500))
                .build();

            let bulkhead = BulkheadLayer::builder()
                .max_concurrent_calls(args.max_concurrent)
                .max_wait_duration(Duration::from_millis(500))
                .build();

            // Response caching for tool calls using SharedCacheLayer.
            // SharedCacheLayer shares the cache store across all layer() calls,
            // so all HTTP sessions share the same cache (unlike regular CacheLayer).
            // The key extractor creates cache keys only for tool calls (tools/call).
            // Other MCP methods (list_tools, initialize, ping) get unique keys
            // that never match, effectively bypassing the cache.
            let cache: SharedCacheLayer<RouterRequest, String, RouterResponse> =
                SharedCacheLayer::builder()
                    .max_size(args.cache_max_size)
                    .ttl(Duration::from_secs(args.cache_ttl_secs))
                    .key_extractor(|req: &RouterRequest| -> String {
                        // Only cache tool calls - create deterministic key from tool name + args
                        match &req.inner {
                            McpRequest::CallTool(CallToolParams {
                                name, arguments, ..
                            }) => {
                                // Serialize arguments to create stable cache key
                                let args_str = serde_json::to_string(arguments).unwrap_or_default();
                                format!("tool:{}:{}", name, args_str)
                            }
                            // For all other requests, use unique key based on request ID
                            // This ensures they're never cached (each request ID is unique)
                            _ => format!("nocache:{:?}", req.id),
                        }
                    })
                    .on_hit(|| tracing::debug!("Cache hit"))
                    .on_miss(|| tracing::debug!("Cache miss"))
                    .build();

            let builder = ServiceBuilder::new()
                // Outer layers (applied first on request, last on response)
                .layer(TimeoutLayer::new(Duration::from_secs(
                    args.request_timeout_secs,
                )))
                .layer(rate_limiter)
                .layer(bulkhead);

            // Conditionally add cache layer
            let transport = if args.cache_enabled {
                HttpTransport::new(router)
                    .disable_origin_validation()
                    .layer(
                        builder
                            .layer(cache)
                            .layer(McpTracingLayer::new())
                            .into_inner(),
                    )
            } else {
                HttpTransport::new(router)
                    .disable_origin_validation()
                    .layer(builder.layer(McpTracingLayer::new()).into_inner())
            };

            transport.serve(&addr).await?;
        }
    }

    Ok(())
}
