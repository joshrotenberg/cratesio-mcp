# Changelog

All notable changes to this project will be documented in this file.

## [0.2.0] - 2026-06-11

### Bug Fixes

- Add #[serde(default)] to optional version params on 5 tools ([#99](https://github.com/joshrotenberg/cratesio-mcp/pull/99))
- Bound rustdoc JSON response size from docs.rs ([#103](https://github.com/joshrotenberg/cratesio-mcp/pull/103))
- Bound get_crate_readme response size ([#108](https://github.com/joshrotenberg/cratesio-mcp/pull/108))
- Bound outbound HTTP calls with a per-request timeout (closes #88) ([#112](https://github.com/joshrotenberg/cratesio-mcp/pull/112))
- Cap retry backoff with saturating math (closes #90) ([#115](https://github.com/joshrotenberg/cratesio-mcp/pull/115))
- Cap unique crates in dependency tree to bound API calls (closes #92) ([#121](https://github.com/joshrotenberg/cratesio-mcp/pull/121))
- Apply timeout/rate-limit/bulkhead middleware to stdio transport (closes #91) ([#123](https://github.com/joshrotenberg/cratesio-mcp/pull/123))

### Documentation

- Add readme field to Cargo.toml package metadata ([#100](https://github.com/joshrotenberg/cratesio-mcp/pull/100))
- Add doc comments to CratesQueryBuilder methods and Sort variants ([#102](https://github.com/joshrotenberg/cratesio-mcp/pull/102))
- Add example to format_number doc comment ([#107](https://github.com/joshrotenberg/cratesio-mcp/pull/107))
- Sync README tool/prompt counts with registrations (closes #76) ([#110](https://github.com/joshrotenberg/cratesio-mcp/pull/110))

### Features

- Log client IP of sampled HTTP requests on the HTTP transport ([#94](https://github.com/joshrotenberg/cratesio-mcp/pull/94))
- Add get_crate_changelog tool ([#72](https://github.com/joshrotenberg/cratesio-mcp/pull/72))
- Add .icon() to authors, user, and user_stats tools ([#101](https://github.com/joshrotenberg/cratesio-mcp/pull/101))
- Normalize MCP tool/prompt surface (closes #74, #77, #83) ([#124](https://github.com/joshrotenberg/cratesio-mcp/pull/124))
- Add get_release_timeline tool (closes #55) ([#125](https://github.com/joshrotenberg/cratesio-mcp/pull/125))
- Auto-reinitialize unknown MCP sessions to stop GET / churn ([#130](https://github.com/joshrotenberg/cratesio-mcp/pull/130))

### Miscellaneous Tasks

- Update rustls-webpki to 0.103.13 to fix RUSTSEC advisories ([#96](https://github.com/joshrotenberg/cratesio-mcp/pull/96))
- Gate HTTP request-origin logging behind --log-requests (default off) ([#98](https://github.com/joshrotenberg/cratesio-mcp/pull/98))
- Add roba.toml worker dispatch config ([#109](https://github.com/joshrotenberg/cratesio-mcp/pull/109))
- Untrack accidentally-committed worker artifacts; gitignore .worktrees and .claudes ([#113](https://github.com/joshrotenberg/cratesio-mcp/pull/113))
- Raise worker max_turns 40->80 and budget cap 2->10 (test-bed dial-in) ([#114](https://github.com/joshrotenberg/cratesio-mcp/pull/114))
- Enforce single-instance Fly deployment (closes #93) ([#126](https://github.com/joshrotenberg/cratesio-mcp/pull/126))
- Upgrade tower-mcp 0.9.1 -> 0.12.0 ([#127](https://github.com/joshrotenberg/cratesio-mcp/pull/127))
- Deploy to Fly only on release tag, not every merge to main ([#129](https://github.com/joshrotenberg/cratesio-mcp/pull/129))

### Testing

- Extend docs/format.rs coverage for module listing and item detail ([#104](https://github.com/joshrotenberg/cratesio-mcp/pull/104))
- Cover find_alternatives in integration router + 404 error path (closes #78) ([#111](https://github.com/joshrotenberg/cratesio-mcp/pull/111))
- Cover composite-tool error and edge paths (closes #85) ([#116](https://github.com/joshrotenberg/cratesio-mcp/pull/116))
- Inline tests for the four new prompts (closes #75) ([#117](https://github.com/joshrotenberg/cratesio-mcp/pull/117))
- Cover 3 untested client endpoints (closes #86) ([#118](https://github.com/joshrotenberg/cratesio-mcp/pull/118))
- Error/empty-path tests for 9 simple tools (part 1, refs #82) ([#119](https://github.com/joshrotenberg/cratesio-mcp/pull/119))
- Error/empty-path tests for 9 simple tools (part 2, closes #82) ([#120](https://github.com/joshrotenberg/cratesio-mcp/pull/120))



## [0.1.4] - 2026-03-19

### Features

- Upgrade tower-mcp to 0.9.1 and enable optional_sessions for HTTP transport ([#63](https://github.com/joshrotenberg/cratesio-mcp/pull/63))
- Add evaluate_dependencies prompt (closes #57) ([#66](https://github.com/joshrotenberg/cratesio-mcp/pull/66))
- Add recommend_crates prompt (closes #58) ([#67](https://github.com/joshrotenberg/cratesio-mcp/pull/67))
- Add migration_guide prompt (closes #59) ([#68](https://github.com/joshrotenberg/cratesio-mcp/pull/68))
- Add retry with exponential backoff for outbound API calls (closes #42) ([#70](https://github.com/joshrotenberg/cratesio-mcp/pull/70))
- Add find_alternatives tool (closes #54) ([#71](https://github.com/joshrotenberg/cratesio-mcp/pull/71))



## [0.1.3] - 2026-02-24

### Bug Fixes

- Disable GitHub release creation in release-plz ([#50](https://github.com/joshrotenberg/cratesio-mcp/pull/50))

### Features

- Add compare_crates tool for side-by-side crate comparison ([#49](https://github.com/joshrotenberg/cratesio-mcp/pull/49))
- Add get_dependency_tree tool for recursive transitive deps ([#45](https://github.com/joshrotenberg/cratesio-mcp/pull/45)) ([#52](https://github.com/joshrotenberg/cratesio-mcp/pull/52))
- Add crate_health_check composite tool ([#53](https://github.com/joshrotenberg/cratesio-mcp/pull/53)) ([#56](https://github.com/joshrotenberg/cratesio-mcp/pull/56))



## [0.1.2] - 2026-02-24

### Bug Fixes

- Add profile.dist for cargo-dist builds ([#47](https://github.com/joshrotenberg/cratesio-mcp/pull/47))



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


