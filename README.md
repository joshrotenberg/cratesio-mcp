# cratesio-mcp

MCP server for querying [crates.io](https://crates.io) -- the Rust package registry.

Built with [tower-mcp](https://github.com/joshrotenberg/tower-mcp).

## Features

### Tools (21)

| Tool | Description |
|------|-------------|
| `search_crates` | Search for crates by name or keywords |
| `get_crate_info` | Detailed crate metadata (description, links, stats) |
| `get_crate_versions` | Version history with release dates and download counts |
| `get_crate_readme` | README content for a crate version |
| `get_dependencies` | Dependencies for a specific version |
| `get_reverse_dependencies` | Crates that depend on a given crate |
| `get_downloads` | Download statistics and trends |
| `get_crate_authors` | Authors listed in Cargo.toml |
| `get_owners` | Crate owners and maintainers |
| `get_user` | User profile by GitHub username |
| `get_summary` | crates.io global statistics |
| `get_categories` | Browse crates.io categories |
| `get_category` | Details for a specific category |
| `get_keywords` | Browse crates.io keywords |
| `get_keyword` | Details for a specific keyword |
| `get_version_downloads` | Daily download stats for a specific version |
| `get_crate_version` | Detailed metadata for a specific version |
| `get_crate_docs` | Browse crate documentation structure from docs.rs |
| `get_doc_item` | Get full docs for a specific item (fn, struct, trait) |
| `search_docs` | Search for items by name within a crate's docs |
| `audit_dependencies` | Check deps against OSV.dev vulnerability database |

### Resources (2)

| Resource | Description |
|----------|-------------|
| `crates://{name}/info` | Crate info as an MCP resource |
| Recent searches | List of recent search queries and results |

### Prompts (2)

| Prompt | Description |
|--------|-------------|
| `analyze_crate` | Guided comprehensive crate analysis |
| `compare_crates` | Compare multiple crates side by side |

### Completions

Auto-complete suggestions for crate names across prompts and resources, seeded with popular crates.

### Middleware

The HTTP transport includes a tower middleware stack:

- **Timeout** -- configurable request timeout
- **Rate limiter** -- token-bucket rate limiting (10 req/s default)
- **Bulkhead** -- concurrent request limit (10 default)
- **Cache** -- shared response cache for tool calls (5 min TTL, 200 entries)
- **Tracing** -- structured logging for all MCP requests

### Transports

- **stdio** -- for use with Claude Desktop, Claude Code, and other MCP clients
- **HTTP/SSE** -- Streamable HTTP with server-sent events (MCP 2025-11-25 spec)

## Installation

Build from source:

```bash
git clone https://github.com/joshrotenberg/cratesio-mcp
cd cratesio-mcp
cargo build --release
```

## Usage

### stdio (default)

```bash
cratesio-mcp
```

### HTTP

```bash
cratesio-mcp --transport http --port 3000
```

### Minimal mode

Use `--minimal` to register only tools (no prompts, resources, or completions). This is useful for Claude Code, which currently has issues discovering tools when prompts and resources are also registered ([anthropics/claude-code#2682](https://github.com/anthropics/claude-code/issues/2682)).

```bash
cratesio-mcp --minimal
```

### CLI options

```text
Usage: cratesio-mcp [OPTIONS]

Options:
  -t, --transport <TRANSPORT>
          Transport to use [default: stdio] [possible values: stdio, http]
      --max-concurrent <MAX_CONCURRENT>
          Maximum concurrent requests (concurrency limit) [default: 10]
      --rate-limit-ms <RATE_LIMIT_MS>
          Rate limit interval between crates.io API calls (in milliseconds) [default: 1000]
  -l, --log-level <LOG_LEVEL>
          Log level [default: info]
      --host <HOST>
          HTTP host to bind to (use 0.0.0.0 for public access) [default: 127.0.0.1]
  -p, --port <PORT>
          HTTP port to bind to [default: 3000]
      --request-timeout-secs <REQUEST_TIMEOUT_SECS>
          Request timeout in seconds (for HTTP transport) [default: 30]
      --minimal
          Minimal mode - only register tools (no prompts, resources, or completions)
      --cache-enabled
          Enable response caching for tool calls (HTTP transport only)
      --cache-ttl-secs <CACHE_TTL_SECS>
          Cache TTL in seconds (how long cached responses are valid) [default: 300]
      --cache-max-size <CACHE_MAX_SIZE>
          Maximum number of cached responses [default: 200]
      --docs-cache-max-entries <DOCS_CACHE_MAX_ENTRIES>
          Maximum number of cached docs.rs rustdoc JSON entries [default: 10]
      --docs-cache-ttl-secs <DOCS_CACHE_TTL_SECS>
          TTL for cached docs.rs rustdoc JSON entries (in seconds) [default: 3600]
```

## MCP client configuration

### Claude Desktop / Claude Code

Add to your MCP configuration:

```json
{
  "cratesio-mcp": {
    "command": "cratesio-mcp"
  }
}
```

Or with the HTTP transport:

```json
{
  "cratesio-mcp": {
    "type": "http",
    "url": "http://localhost:3000/"
  }
}
```

## Library usage

The crate also exposes a library with a typed async client for the crates.io API:

```rust
use std::time::Duration;
use cratesio_mcp::client::{CratesIoClient, CratesQuery, Sort};

let client = CratesIoClient::new("my-app", Duration::from_secs(1))?;

// Search for crates
let query = CratesQuery::builder()
    .search("tower")
    .sort(Sort::Downloads)
    .build();
let results = client.crates(query).await?;

// Get crate details
let info = client.get_crate("tower-mcp").await?;

// Authenticated operations
let client = client.with_auth("your-api-token");
let me = client.me().await?;
```

The client covers 46 endpoints across crates, versions, owners, categories, keywords, users, teams, tokens, publishing, and trusted publishing.

## Roadmap

- [x] Custom crates.io API client (46 endpoints)
- [x] Library crate extraction
- [x] 21 MCP tools
- [x] Resources, prompts, and completions
- [x] Tower middleware stack (timeout, rate limit, bulkhead, cache)
- [x] stdio and HTTP transports
- [x] CI pipeline ([#5](https://github.com/joshrotenberg/cratesio-mcp/issues/5))
- [x] docs.rs integration ([#2](https://github.com/joshrotenberg/cratesio-mcp/issues/2))
- [x] Dependency security audit via OSV.dev ([#7](https://github.com/joshrotenberg/cratesio-mcp/issues/7))
- [ ] Publish to crates.io ([#4](https://github.com/joshrotenberg/cratesio-mcp/issues/4))
- [ ] Fly.io deployment ([#6](https://github.com/joshrotenberg/cratesio-mcp/issues/6))
- [ ] Feature flag analysis tool ([#15](https://github.com/joshrotenberg/cratesio-mcp/issues/15))
- [ ] New resources: readme, docs ([#16](https://github.com/joshrotenberg/cratesio-mcp/issues/16))
- [ ] User download stats tool ([#18](https://github.com/joshrotenberg/cratesio-mcp/issues/18))

## License

MIT OR Apache-2.0
