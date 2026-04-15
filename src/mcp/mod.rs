//! MCP stdio server exposing the same 7-tool surface as the CLI.
//!
//! # Design
//!
//! Two tool sets, chosen at startup by the `--allow-write` flag:
//!
//! - **Read-only** (default): `get_page`, `get_data_source`,
//!   `query_data_source`, `search` (4 tools).
//! - **Full** (`--allow-write`): above + `create_page`, `update_page`,
//!   `create_data_source` (7 tools).
//!
//! Per the security model established in the plan (§7), read-only is
//! the safe default. The write surface exists because `BlueNode`
//! integration needs `create_data_source` — the whole reason this
//! crate exists — but any write call is audited (append-only JSONL
//! via `NOTION_CLI_AUDIT_LOG` env or `--audit-log` flag).
//!
//! # Schemas
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
pub mod handlers;
pub mod params;
pub mod server;

pub use server::{run_read_only, run_with_write};
