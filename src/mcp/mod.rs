//! MCP stdio server — three tiers, selected by flag:
//!
//! | Flag              | Tier           | Tools |
//! |-------------------|----------------|-------|
//! | (none)            | Read-only      | 6     |
//! | `--allow-write`   | Runtime writes | 12    |
//! | `--allow-admin`   | Admin ops      | 12 + admin lifecycle |
//!
//! # Module split (D5)
//!
//! Each tier has its own `#[tool_router]` impl in its own file —
//! module boundary is the invariant. An admin-only tool added to
//! the wrong file won't leak into a lower-privilege tier because
//! the macro sees only what's in its own impl block.
//!
//! - [`server_ro`] — `NotionReadOnly`, [`server_ro::run_read_only`]
//! - [`server_write`] — `NotionWrite`, [`server_write::run_with_write`]
//! - [`server_admin`] — `NotionAdmin`, [`server_admin::run_with_admin`]
//! - [`common`] — shared `Inner` state + `to_result` helper
//!
//! # Schemas (unchanged from v0.2)
//!
//! MCP tool `input_schema` is generated via `schemars 1.2`. Param
//! structs keep complex Notion-shape fields (filters, property maps)
//! as `serde_json::Value` so the emitted schema is flat — Anthropic
//! tool-use and Hermes Gemma parsers both degrade on deep `oneOf`
//! with `$ref` recursion.
//!
//! # Output envelope
//!
//! Every response is wrapped in the untrusted envelope
//! (see [`crate::output`]), so the calling LLM's system prompt can
//! treat the payload as data, never instructions.

pub mod audit;
pub mod common;
pub mod handlers;
pub mod params;
pub mod server_admin;
pub mod server_ro;
pub mod server_write;

pub use server_admin::run_with_admin;
pub use server_ro::run_read_only;
pub use server_write::run_with_write;
