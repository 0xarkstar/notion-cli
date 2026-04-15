//! Notion API version pin.
//!
//! Pinned to 2026-03-11 (latest as of crate creation). The stated
//! project goal is replacing `@notionhq/notion-mcp-server` whose
//! `create_a_data_source` endpoint breaks on API versions
//! 2025-09-03+. Version is sent as the `Notion-Version` header on
//! every request.

pub const NOTION_API_VERSION: &str = "2026-03-11";
pub const NOTION_API_BASE: &str = "https://api.notion.com";
