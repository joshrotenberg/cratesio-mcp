# cratesio-mcp

[![Crates.io](https://img.shields.io/crates/v/cratesio-mcp.svg)](https://crates.io/crates/cratesio-mcp)
[![Documentation](https://docs.rs/cratesio-mcp/badge.svg)](https://docs.rs/cratesio-mcp)
[![CI](https://github.com/joshrotenberg/cratesio-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/joshrotenberg/cratesio-mcp/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/cratesio-mcp.svg)](https://github.com/joshrotenberg/cratesio-mcp#license)
[![MSRV](https://img.shields.io/crates/msrv/cratesio-mcp.svg)](https://github.com/joshrotenberg/cratesio-mcp)

[MCP](https://modelcontextprotocol.io) server for querying [crates.io](https://crates.io) -- the Rust package registry. Built with [tower-mcp](https://github.com/joshrotenberg/tower-mcp).

Gives your AI agent access to crate search, documentation, dependency analysis, download stats, and security auditing -- everything it needs to make informed decisions about Rust dependencies.

## Quick start

### Hosted (no install)

A public instance is running at **https://cratesio-mcp.fly.dev/**. Add to your MCP client config:

```json
{
  "mcpServers": {
    "cratesio-mcp": {
      "type": "http",
      "url": "https://cratesio-mcp.fly.dev/"
    }
  }
}
```

### Install from crates.io

```bash
cargo install cratesio-mcp
```

### Build from source

```bash
git clone https://github.com/joshrotenberg/cratesio-mcp
cd cratesio-mcp
cargo install --path .
```

### Docker

```bash
docker run -p 3000:3000 ghcr.io/joshrotenberg/cratesio-mcp:latest
```

## MCP client configuration

### Claude Code (stdio)

```json
{
  "mcpServers": {
    "cratesio-mcp": {
      "command": "cratesio-mcp"
    }
  }
}
```

### Claude Code (HTTP, local or remote)

```json
{
  "mcpServers": {
    "cratesio-mcp": {
      "type": "http",
      "url": "http://localhost:3000/"
    }
  }
}
```

### Claude Desktop

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "cratesio-mcp": {
      "command": "cratesio-mcp"
    }
  }
}
```

## What's included

### Tools (23)

| Tool | Description |
|------|-------------|
| `search_crates` | Search for crates by name or keywords |
| `get_crate_info` | Detailed crate metadata (description, links, stats) |
| `get_crate_versions` | Version history with release dates and download counts |
| `get_crate_version` | Detailed metadata for a specific version |
| `get_crate_readme` | README content for a crate version |
| `get_crate_features` | Feature flags and their sub-feature activations |
| `get_crate_docs` | Browse documentation structure from docs.rs |
| `get_doc_item` | Full docs for a specific item (fn, struct, trait) |
| `search_docs` | Search for items by name within a crate's docs |
| `get_dependencies` | Dependencies for a specific version |
| `get_reverse_dependencies` | Crates that depend on a given crate |
| `audit_dependencies` | Check deps against OSV.dev vulnerability database |
| `get_downloads` | Download statistics and trends |
| `get_version_downloads` | Daily download stats for a specific version |
| `get_crate_authors` | Authors listed in Cargo.toml |
| `get_owners` | Crate owners and maintainers |
| `get_user` | User profile by GitHub username |
| `get_user_stats` | Total download stats for a user's crates |
| `get_summary` | crates.io global statistics |
| `get_categories` | Browse crates.io categories |
| `get_category` | Details for a specific category |
| `get_keywords` | Browse crates.io keywords |
| `get_keyword` | Details for a specific keyword |

### Resources (4)

| Resource | Description |
|----------|-------------|
| `crates://{name}/info` | Crate metadata |
| `crates://{name}/readme` | Crate README content |
| `crates://{name}/docs` | Documentation structure |
| Recent searches | Recent search queries and results |

### Prompts (2)

| Prompt | Description |
|--------|-------------|
| `analyze_crate` | Guided comprehensive crate analysis |
| `compare_crates` | Compare multiple crates side by side |

## Transports

- **stdio** (default) -- for Claude Desktop, Claude Code, and other MCP clients
- **HTTP/SSE** -- Streamable HTTP with server-sent events (MCP 2025-11-25 spec)

```bash
# stdio (default)
cratesio-mcp

# HTTP
cratesio-mcp --transport http --port 3000
```

The HTTP transport includes a [tower](https://github.com/tower-rs/tower) middleware stack: timeout, rate limiting, bulkhead concurrency control, optional response caching, and structured tracing.

## Library usage

The crate also exposes a typed async client for the crates.io API:

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
```

The client covers 46 endpoints across crates, versions, owners, categories, keywords, users, teams, tokens, publishing, and trusted publishing.

## License

MIT OR Apache-2.0
