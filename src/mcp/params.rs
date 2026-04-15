//! MCP tool parameter structs.
//!
//! Kept deliberately flat. IDs are plain strings (validated at call
//! time into newtypes), and complex Notion-shape fields
//! (filter expression, property maps, sort arrays) stay as
//! `serde_json::Value` so the JSON Schema emitted by schemars is
//! shallow and agent-friendly.

use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// === Read-only params =====================================================

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetPageParams {
    /// Notion page ID — 32 hex chars (optionally dashed) or a Notion URL.
    pub page_id: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetDataSourceParams {
    /// Notion data source ID.
    pub data_source_id: String,
}

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct QueryDataSourceParams {
    /// Notion data source ID — the per-table schema container.
    pub data_source_id: String,
    /// Notion filter expression. See
    /// <https://developers.notion.com/reference/post-database-query-filter>
    /// for the full grammar. Example:
    /// `{"property": "Done", "checkbox": {"equals": true}}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
    /// Array of sort criteria. Example:
    /// `[{"property": "Name", "direction": "ascending"}]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sorts: Option<serde_json::Value>,
    /// Opaque pagination cursor from a previous response's `next_cursor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_cursor: Option<String>,
    /// Results per page, 1-100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u8>,
}

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct SearchParams {
    /// Search query string. Empty string matches everything the
    /// integration has access to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Filter object. Example:
    /// `{"property": "object", "value": "page"}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
    /// Sort object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u8>,
}

// === Write params =========================================================

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CreatePageParams {
    /// Parent data source ID (for pages inside a database). Mutually
    /// exclusive with `parent_page_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_data_source_id: Option<String>,
    /// Parent page ID (for sub-pages). Mutually exclusive with
    /// `parent_data_source_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_page_id: Option<String>,
    /// Property values keyed by name. Each value must match the Notion
    /// property-value wire format, e.g.
    /// `{"Done": {"type": "checkbox", "checkbox": true},
    ///   "Name": {"type": "title", "title": [{"type": "text",
    ///            "text": {"content": "Hello"}}]}}`.
    /// Call `notion-cli schema property-value` for the full shape.
    pub properties: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UpdatePageParams {
    pub page_id: String,
    /// Properties to update. Same shape as in `create_page`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Value>,
    /// Set `archived` flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    /// Set `in_trash` flag. Preferred over `archived` on API 2025-09-03+.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_trash: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CreateDataSourceParams {
    /// Parent database ID. The data source will live inside this
    /// database container.
    pub parent_database_id: String,
    /// Plain-text title for the data source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Property schemas. Example:
    /// `{"Name": {"title": {}}, "Tags": {"multi_select": {"options": []}}}`.
    pub properties: serde_json::Value,
}
