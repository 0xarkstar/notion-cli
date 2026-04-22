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
    /// Optional page body as a JSON array of block bodies.
    /// Preferred over create + append — sets up the page and its
    /// content in one API call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<serde_json::Value>,
    /// Icon: emoji literal (e.g. `"🚀"`) or `http(s)://` URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Cover image URL (`http(s)://`, URL-only — Notion covers have no
    /// emoji form).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
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
    /// Icon: emoji literal or `http(s)://` URL, or `"none"` to clear.
    /// Absent string → leave unchanged (tristate, per D11).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Cover URL (`http(s)://`) or `"none"` to clear. Absent → leave
    /// unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
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

// === Admin tier params ====================================================

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct PageMoveParams {
    /// Page ID (32-hex, dashed, or URL) to move.
    pub page_id: String,
    /// New parent page. Mutually exclusive with
    /// `target_data_source_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_page_id: Option<String>,
    /// New parent data source. Mutually exclusive with
    /// `target_page_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_data_source_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DsUpdateParams {
    /// Data source ID or URL.
    pub data_source_id: String,
    /// Operation to perform. One of:
    /// `add_property`, `remove_property`, `rename_property`,
    /// `add_option`, `bulk`.
    ///
    /// - `add_property` needs: `name`, `schema`.
    /// - `remove_property` needs: `name`, `confirm=true`, AND
    ///   `NOTION_CLI_ADMIN_CONFIRMED=1` env (D1 two-factor gate).
    /// - `rename_property` needs: `name` (old), `new_name`.
    /// - `add_option` needs: `property`, `kind`
    ///   (`select` | `multi_select` | `status`), `option` (object
    ///   with `name` and optional `color`).
    /// - `bulk` needs: `body` (full `UpdateDataSourceRequest` JSON).
    ///   Non-atomic — caller accepts partial failure.
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_name: Option<String>,
    /// Property schema body (shape: `PropertySchema` JSON).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<serde_json::Value>,
    /// Target property name for `add_option`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,
    /// Property kind for `add_option`: `select`, `multi_select`, `status`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Option body for `add_option` (object with `name`, optional `color`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub option: Option<serde_json::Value>,
    /// Full bulk body for `bulk` action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
    /// Two-factor gate bit for destructive actions (D1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DsAddRelationParams {
    /// Source data source ID or URL — the DS gaining a relation property.
    pub source_data_source_id: String,
    /// Name for the new relation property on the source.
    pub name: String,
    /// Target data source ID. Required unless `self` is true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_data_source_id: Option<String>,
    /// Two-way relation: Notion creates a reciprocal property on the
    /// target data source with this name. Mutually exclusive with
    /// `one_way` and `self`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backlink: Option<String>,
    /// One-way relation (`single_property`). Mutually exclusive with
    /// `backlink` and `self`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub one_way: Option<bool>,
    /// Self-referential relation (same source and target DS).
    /// Skips pre-flight GET on the target.
    #[serde(default, rename = "self", skip_serializing_if = "Option::is_none")]
    pub self_ref: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DbCreateParams {
    /// Parent page ID (32-hex or dashed, or a page URL). Must be a
    /// regular page — databases cannot parent other databases.
    pub parent_page_id: String,
    /// Database title (plain text; multi-run rich text is CLI-only).
    pub title: String,
    /// Initial properties schema keyed by property name. The map
    /// must contain at least one `title`-typed property. Example:
    /// `{"Name": {"type": "title", "title": {}}, "Priority":
    /// {"type": "select", "select": {"options": [{"name": "High"}]}}}`.
    /// Call `notion-cli schema property-schema --pretty` for the full shape.
    pub properties: serde_json::Value,
    /// Icon: emoji literal (e.g. `"🚀"`) OR `http(s)://` URL. The handler
    /// parses URLs as `external`, everything else as `emoji`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Cover image URL (URL only — Notion has no emoji-cover form).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    /// Mark as inline (rendered inside the parent page).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_inline: Option<bool>,
}

// === Block params =========================================================

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetBlockParams {
    /// Notion block ID.
    pub block_id: String,
}

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct ListBlockChildrenParams {
    /// Parent block ID (a page ID is also a block ID).
    pub block_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_cursor: Option<String>,
    /// Results per page (1-100).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u8>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AppendBlockChildrenParams {
    /// Parent block ID to append children into.
    pub block_id: String,
    /// JSON array of block bodies. Each element must have `type` and
    /// the type-specific content, e.g.
    /// `[{"type":"paragraph","paragraph":{"rich_text":[{"type":"text","text":{"content":"Hi"}}]}}]`.
    /// Call `notion-cli schema rich-text` / `schema property-value` for
    /// the full nested shape. Supported block types:
    /// `paragraph`, `heading_1`/`heading_2`/`heading_3`,
    /// `bulleted_list_item`, `numbered_list_item`, `to_do`, `toggle`,
    /// `code`, `quote`, `callout`, `divider`.
    pub children: serde_json::Value,
    /// Optional: append after this sibling block ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UpdateBlockParams {
    pub block_id: String,
    /// JSON body of a single block. Top-level `type` must match the
    /// block's existing type (Notion does not allow type change on
    /// update). The shape is flat — `type` and the type-specific
    /// content field are siblings. Examples:
    ///
    /// - Paragraph:
    ///   `{"type":"paragraph","paragraph":{"rich_text":[{"type":"text","text":{"content":"new"}}],"color":"default"}}`
    /// - To-do with checked toggle:
    ///   `{"type":"to_do","to_do":{"rich_text":[{"type":"text","text":{"content":"task"}}],"checked":true,"color":"default"}}`
    /// - Heading:
    ///   `{"type":"heading_1","heading_1":{"rich_text":[{"type":"text","text":{"content":"Title"}}],"color":"default","is_toggleable":false}}`
    ///
    /// Omit this field to only change `archived`/`in_trash`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_trash: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DeleteBlockParams {
    pub block_id: String,
}
