# cratesio-mcp

MCP server for querying [crates.io](https://crates.io) -- the Rust package registry.

Built with [tower-mcp](https://github.com/joshrotenberg/tower-mcp).

## Features

### Tools (20)

| Tool | Description | Status |
|------|-------------|--------|
| `search_crates` | Search for crates by name or keywords | Implemented |
| `get_crate_info` | Detailed crate metadata (description, links, stats) | Implemented |
| `get_crate_versions` | Version history with release dates and download counts | Implemented |
| `get_crate_readme` | README content for a crate version | Implemented |
| `get_dependencies` | Dependencies for a specific version | Implemented |
| `get_reverse_dependencies` | Crates that depend on a given crate | Implemented |
| `get_downloads` | Download statistics and trends | Implemented |
| `get_crate_authors` | Authors listed in Cargo.toml | Implemented |
| `get_owners` | Crate owners and maintainers | Implemented |
| `get_user` | User profile by GitHub username | Implemented |
| `get_summary` | crates.io global statistics | Implemented |
| `get_categories` | Browse crates.io categories | Implemented |
| `get_category` | Details for a specific category | Implemented |
| `get_keywords` | Browse crates.io keywords | Implemented |
| `get_keyword` | Details for a specific keyword | Implemented |
| `get_version_downloads` | Daily download stats for a specific version | Implemented |
| `get_crate_version` | Detailed metadata for a specific version | Implemented |
| `get_crate_docs` | Browse crate documentation structure from docs.rs | Implemented |
| `get_doc_item` | Get full docs for a specific item (fn, struct, trait) | Implemented |
| `search_docs` | Search for items by name within a crate's docs | Implemented |
| `audit_dependencies` | Check deps against RustSec advisory DB | [Planned (#7)](https://github.com/joshrotenberg/cratesio-mcp/issues/7) |

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

```bash
cargo install cratesio-mcp
```

Or build from source:

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

### CLI options

```text
Options:
  -t, --transport <TRANSPORT>              Transport: stdio or http [default: stdio]
      --max-concurrent <N>                 Max concurrent requests [default: 10]
      --rate-limit-ms <MS>                 Rate limit interval in ms [default: 1000]
  -l, --log-level <LEVEL>                  Log level [default: info]
      --host <HOST>                        HTTP bind address [default: 127.0.0.1]
  -p, --port <PORT>                        HTTP port [default: 3000]
      --request-timeout-secs <S>           Request timeout [default: 30]
      --minimal                            Tools only (no prompts/resources/completions)
      --cache-enabled                      Enable response caching [default: true]
      --cache-ttl-secs <S>                 Cache TTL [default: 300]
      --cache-max-size <N>                 Max cached responses [default: 200]
      --docs-cache-max-entries <N>         Max cached docs.rs entries [default: 10]
      --docs-cache-ttl-secs <S>            docs.rs cache TTL [default: 3600]
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
- [x] 20 MCP tools
- [x] Resources, prompts, and completions
- [x] Tower middleware stack (timeout, rate limit, bulkhead, cache)
- [x] stdio and HTTP transports
- [x] docs.rs integration ([#2](https://github.com/joshrotenberg/cratesio-mcp/issues/2))
- [ ] Dependency security audit via RustSec ([#7](https://github.com/joshrotenberg/cratesio-mcp/issues/7))
- [ ] CI pipeline ([#5](https://github.com/joshrotenberg/cratesio-mcp/issues/5))
- [ ] Publish to crates.io ([#4](https://github.com/joshrotenberg/cratesio-mcp/issues/4))
- [ ] Fly.io deployment ([#6](https://github.com/joshrotenberg/cratesio-mcp/issues/6))

## License

MIT OR Apache-2.0
