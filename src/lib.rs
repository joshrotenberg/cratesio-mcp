//! An MCP server for crates.io, plus a standalone async crates.io API client.
//!
//! # Cargo features
//!
//! - **`mcp`** (default) -- the full MCP server: `tools`, `prompts`,
//!   `resources`, shared `state`, and both transports. Required by the
//!   `cratesio-mcp` binary.
//! - **`client`** -- the [`client`] (crates.io / docs.rs / OSV) and [`docs`]
//!   library only, with none of the MCP server dependencies. Depend on it with:
//!   `cratesio-mcp = { version = "...", default-features = false, features = ["client"] }`.

pub mod client;
pub mod docs;

// MCP server modules -- gated behind the `mcp` feature (the default).
// Disable with `default-features = false, features = ["client"]` to use just
// the crates.io / docs.rs / OSV client library.
#[cfg(feature = "mcp")]
pub mod prompts;
#[cfg(feature = "mcp")]
pub mod resources;
#[cfg(feature = "mcp")]
pub mod state;
#[cfg(feature = "mcp")]
pub mod tools;
