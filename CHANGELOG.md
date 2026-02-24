# Changelog

All notable changes to this project will be documented in this file.

## [0.1.1] - 2026-02-24

### Bug Fixes

- Decompress gzip responses from docs.rs rustdoc JSON endpoint ([#22](https://github.com/joshrotenberg/cratesio-mcp/pull/22))
- Downgrade rustdoc-types to 0.56 to match docs.rs format version ([#24](https://github.com/joshrotenberg/cratesio-mcp/pull/24))
- Show version strings instead of IDs in get_downloads ([#40](https://github.com/joshrotenberg/cratesio-mcp/pull/40))

### Documentation

- Add README with implemented and planned features ([#9](https://github.com/joshrotenberg/cratesio-mcp/pull/9))
- Add LICENSE files and clean up README ([#28](https://github.com/joshrotenberg/cratesio-mcp/pull/28))
- Rewrite README for release ([#41](https://github.com/joshrotenberg/cratesio-mcp/pull/41))

### Features

- Custom client library, lib crate extraction, and 7 new tools ([#1](https://github.com/joshrotenberg/cratesio-mcp/pull/1))
- Add docs.rs integration tools (get_crate_docs, get_doc_item, search_docs) ([#11](https://github.com/joshrotenberg/cratesio-mcp/pull/11))
- Add audit_dependencies tool via OSV.dev API ([#14](https://github.com/joshrotenberg/cratesio-mcp/pull/14))
- Add get_crate_features tool for feature flag analysis ([#31](https://github.com/joshrotenberg/cratesio-mcp/pull/31))
- Add readme and docs resource templates ([#32](https://github.com/joshrotenberg/cratesio-mcp/pull/32))
- Add get_user_stats tool for user download statistics ([#34](https://github.com/joshrotenberg/cratesio-mcp/pull/34))
- Add release-plz and cargo-dist for automated releases ([#35](https://github.com/joshrotenberg/cratesio-mcp/pull/35))
- Add Docker image build and publish to ghcr.io ([#37](https://github.com/joshrotenberg/cratesio-mcp/pull/37))
- Add Fly.io deploy workflow with MCP protocol verification ([#38](https://github.com/joshrotenberg/cratesio-mcp/pull/38))

### Miscellaneous Tasks

- Add GitHub Actions CI workflow ([#12](https://github.com/joshrotenberg/cratesio-mcp/pull/12))
- Add .mcp.json for local development ([#13](https://github.com/joshrotenberg/cratesio-mcp/pull/13))

### Testing

- Add 28 wiremock tests for authenticated write operations ([#29](https://github.com/joshrotenberg/cratesio-mcp/pull/29))
- Add 27 MCP integration tests using TestClient + wiremock ([#30](https://github.com/joshrotenberg/cratesio-mcp/pull/30))
- Add retroactive test coverage for state, crate_info resource, and uncovered tools ([#33](https://github.com/joshrotenberg/cratesio-mcp/pull/33))


