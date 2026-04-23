//! Notion `/v1/data_sources/*` endpoints.
//!
//! # The bug this crate exists to fix
//!
//! `@notionhq/notion-mcp-server` `create_a_data_source` call fails
//! with `validation_error` on Notion API 2025-09-03+. This module
//! implements the correct routing against API 2026-03-11: data
//! sources live under `/v1/data_sources`, not under
//! `/v1/databases/…` as the upstream assumed.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::client::NotionClient;
use crate::api::error::ApiError;
use crate::api::pagination::PaginatedResponse;
use crate::types::common::SelectOption;
use crate::types::page::Page;
use crate::types::property::PropertyValue;
use crate::types::property_schema::PropertySchema;
use crate::types::rich_text::RichText;
use crate::types::sort::SortCriterion;
use crate::types::{DataSource, DatabaseParentRef};
use crate::validation::{DataSourceId, DatabaseId};

// === Requests =============================================================

/// Parent reference for a new data source.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CreateDataSourceParent {
    #[serde(rename = "database_id")]
    Database { database_id: DatabaseId },
}

/// Request body for `POST /v1/data_sources`.
///
/// `properties` is left as raw JSON because the schema-definition
/// surface (`{ "Name": { "title": {} }, "Tags": { "multi_select": {} } }`)
/// is complex; typed coverage is deferred to v0.2.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateDataSourceRequest {
    pub parent: CreateDataSourceParent,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub title: Vec<RichText>,
    pub properties: serde_json::Value,
}

/// Direction of a relation property created via `ds add-relation`.
///
/// - `OneWay` — Notion `single_property` relation; no backlink
///   property is created on the target data source.
/// - `Dual(synced_name)` — Notion `dual_property` relation; Notion
///   auto-creates a reciprocal property on the target with the
///   given name (D7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelationDirection {
    OneWay,
    Dual(String),
}

/// Kinds of property that support an option list and hence
/// `ds update add-option`. Notion merges options by name on PATCH —
/// existing options are preserved.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SelectKind {
    Select,
    MultiSelect,
    Status,
}

impl SelectKind {
    #[must_use]
    pub fn wire_key(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::MultiSelect => "multi_select",
            Self::Status => "status",
        }
    }

    /// Parse the wire discriminator (`select`, `multi_select`, `status`).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "select" => Ok(Self::Select),
            "multi_select" => Ok(Self::MultiSelect),
            "status" => Ok(Self::Status),
            other => Err(format!(
                "unknown option-capable kind '{other}' (expected select, multi_select, status)"
            )),
        }
    }
}

/// Body for `PATCH /v1/data_sources/{id}`.
///
/// `properties` is a map of property-name → delta. A delta can be:
/// - full `PropertySchema` JSON — add or redefine the property
/// - `{"name": "NewName"}` — rename the property
/// - JSON `null` — remove the property
/// - `{"<kind>": {"options": [...]}}` — append options to a select
///   (Notion merges by name, existing options preserved)
///
/// The CLI/MCP surface enforces one-delta-per-invocation by default
/// (D2); the library API below is multi-delta capable for advanced
/// consumers that accept the non-atomic semantics.
#[derive(Debug, Clone, Serialize, JsonSchema, Default)]
pub struct UpdateDataSourceRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<Vec<RichText>>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub properties: serde_json::Map<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_trash: Option<bool>,
}

impl UpdateDataSourceRequest {
    /// Single-delta: add a new property to the schema.
    pub fn add_property(
        name: &str,
        schema: &PropertySchema,
    ) -> Result<Self, serde_json::Error> {
        let mut props = serde_json::Map::new();
        props.insert(name.to_string(), serde_json::to_value(schema)?);
        Ok(Self { properties: props, ..Default::default() })
    }

    /// Single-delta: remove a property (Notion PATCH accepts `null`
    /// as the tombstone value).
    #[must_use]
    pub fn remove_property(name: &str) -> Self {
        let mut props = serde_json::Map::new();
        props.insert(name.to_string(), serde_json::Value::Null);
        Self { properties: props, ..Default::default() }
    }

    /// Single-delta: rename a property.
    /// Wire shape: `{"OldName": {"name": "NewName"}}`.
    #[must_use]
    pub fn rename_property(old: &str, new: &str) -> Self {
        let mut props = serde_json::Map::new();
        props.insert(
            old.to_string(),
            serde_json::json!({"name": new}),
        );
        Self { properties: props, ..Default::default() }
    }

    /// Single-delta: append an option to a select / multi-select /
    /// status property. Notion merges by option name — pre-existing
    /// options are preserved.
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_option(prop_name: &str, kind: SelectKind, option: SelectOption) -> Self {
        let mut props = serde_json::Map::new();
        let key = kind.wire_key();
        let body = serde_json::json!({
            "type": key,
            key: { "options": [option] }
        });
        props.insert(prop_name.to_string(), body);
        Self { properties: props, ..Default::default() }
    }

    /// Single-delta convenience: add a relation property. Uses
    /// `data_source_id` (not `database_id`) per the API 2025-09-03+
    /// migration. Direction is either one-way (`single_property`) or
    /// two-way (`dual_property` with a backlink name).
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_relation_property(
        prop_name: &str,
        target: DataSourceId,
        direction: RelationDirection,
    ) -> Self {
        let inner = match direction {
            RelationDirection::OneWay => serde_json::json!({
                "data_source_id": target,
                "type": "single_property",
                "single_property": {}
            }),
            RelationDirection::Dual(backlink) => serde_json::json!({
                "data_source_id": target,
                "type": "dual_property",
                "dual_property": {"synced_property_name": backlink}
            }),
        };
        let body = serde_json::json!({
            "type": "relation",
            "relation": inner
        });
        let mut props = serde_json::Map::new();
        props.insert(prop_name.to_string(), body);
        Self { properties: props, ..Default::default() }
    }

    /// Escape hatch (`--bulk`): take a caller-supplied JSON body
    /// verbatim. Caller accepts non-atomic semantics (partial-failure
    /// leaves schema mid-state per D2).
    pub fn from_bulk(body: &serde_json::Value) -> Result<Self, String> {
        let obj = body
            .as_object()
            .ok_or_else(|| "bulk body must be a JSON object".to_string())?;
        let mut req = Self::default();
        if let Some(t) = obj.get("title").cloned() {
            req.title = Some(
                serde_json::from_value(t).map_err(|e| format!("title: {e}"))?,
            );
        }
        if let Some(p) = obj.get("properties").and_then(serde_json::Value::as_object) {
            req.properties.clone_from(p);
        }
        if let Some(a) = obj.get("archived").and_then(serde_json::Value::as_bool) {
            req.archived = Some(a);
        }
        if let Some(t) = obj.get("in_trash").and_then(serde_json::Value::as_bool) {
            req.in_trash = Some(t);
        }
        Ok(req)
    }
}

/// Request body for `POST /v1/data_sources/{id}/query`.
#[derive(Debug, Clone, Serialize, Default, JsonSchema)]
pub struct QueryDataSourceRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sorts: Vec<SortCriterion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u8>,
}

// === API surface ==========================================================

impl NotionClient {
    /// `POST /v1/data_sources`.
    ///
    /// This is the exact endpoint that fails in
    /// `@notionhq/notion-mcp-server` on API 2025-09-03+.
    pub async fn create_data_source(
        &self,
        req: &CreateDataSourceRequest,
    ) -> Result<DataSource, ApiError> {
        self.post("/data_sources", req).await
    }

    /// `GET /v1/data_sources/{id}`.
    pub async fn retrieve_data_source(
        &self,
        id: &DataSourceId,
    ) -> Result<DataSource, ApiError> {
        self.get(&format!("/data_sources/{id}")).await
    }

    /// `POST /v1/data_sources/{id}/query`.
    pub async fn query_data_source(
        &self,
        id: &DataSourceId,
        req: &QueryDataSourceRequest,
    ) -> Result<PaginatedResponse<Page>, ApiError> {
        self.post(&format!("/data_sources/{id}/query"), req).await
    }

    /// `PATCH /v1/data_sources/{id}` — mutate schema.
    ///
    /// Notion is non-transactional across multi-property deltas; CLI
    /// surface enforces single-property per invocation by default
    /// (D2). Library callers that opt into multi-delta accept the
    /// partial-failure semantics.
    pub async fn update_data_source(
        &self,
        id: &DataSourceId,
        req: &UpdateDataSourceRequest,
    ) -> Result<DataSource, ApiError> {
        self.patch(&format!("/data_sources/{id}"), req).await
    }
}

// === Helpers ==============================================================

impl CreateDataSourceParent {
    #[must_use]
    pub fn database(id: DatabaseId) -> Self {
        Self::Database { database_id: id }
    }
}

impl From<DatabaseParentRef> for CreateDataSourceParent {
    fn from(r: DatabaseParentRef) -> Self {
        Self::Database { database_id: r.database_id }
    }
}

/// Convenience: build a write-safe property map from a borrowed
/// [`PropertyValue`] iterator — the expected shape for Notion write
/// requests (see module docs on `Property::Raw` safety).
#[must_use]
pub fn property_map<I>(iter: I) -> HashMap<String, PropertyValue>
where
    I: IntoIterator<Item = (String, PropertyValue)>,
{
    iter.into_iter().collect()
}
