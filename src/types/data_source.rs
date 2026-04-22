//! Notion data source — the per-table schema container introduced in
//! API 2025-09-03 to support multi-source databases.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::property_schema::Schema;
use crate::types::rich_text::RichText;
use crate::validation::{DataSourceId, DatabaseId};

/// A Notion data source — the per-table schema container introduced
/// in API 2025-09-03.
///
/// The wire shape has stabilised with `parent.{type, database_id}`
/// pointing at the owning database. We keep `parent` as raw JSON
/// because it is untyped across the different parent kinds (database
/// / page / workspace); surface-side parsing happens at use sites.
///
/// # `properties` typing (v0.3 BREAKING)
///
/// `properties` is `HashMap<String, Schema>` as of v0.3. v0.2 read
/// this as `HashMap<String, serde_json::Value>`. Consumers who did
/// `.get(name).and_then(|v| v.get("type"))` must migrate to matching
/// on [`Schema::Known`] / [`Schema::Raw`]. The `Raw` fallback
/// preserves forward-compat for unknown property types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DataSource {
    pub id: DataSourceId,
    pub created_time: String,
    pub last_edited_time: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Vec<RichText>,
    #[serde(default)]
    pub properties: HashMap<String, Schema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<serde_json::Value>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub in_trash: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_url: Option<String>,
}

/// Typed reference to a parent database, for building write requests
/// that need a `{type: "database_id", database_id: ...}` shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DatabaseParentRef {
    pub database_id: DatabaseId,
}
