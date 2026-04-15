//! notion-cli: Agent-First Notion CLI and MCP server.
//!
//! Crate is split into three layers:
//! - [`validation`] — newtype IDs with format parsing (no URL, strict) and
//!   URL-accepting constructors. IDs carry no filesystem semantics.
//! - [`types`] — serde-deserialisable Notion API response types. The
//!   [`types::Property`] wrapper provides graceful degradation for unknown
//!   property types via an `untagged` fallback to [`serde_json::Value`].
//! - [`error`] — crate error hierarchy.

pub mod error;
pub mod types;
pub mod validation;
