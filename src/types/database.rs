//! Notion database object.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::property_schema::Schema;
use crate::types::rich_text::RichText;
use crate::validation::{DataSourceId, DatabaseId};

/// # `properties` typing (v0.3 BREAKING)
///
/// `properties` is `HashMap<String, Schema>` as of v0.3. v0.2 read
/// this as `HashMap<String, serde_json::Value>`. See
/// [`DataSource::properties`](crate::types::data_source::DataSource::properties)
/// for migration notes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Database {
    pub id: DatabaseId,
    pub created_time: String,
    pub last_edited_time: String,
    #[serde(default)]
    pub title: Vec<RichText>,
    #[serde(default)]
    pub description: Vec<RichText>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub in_trash: bool,
    /// Typed property schemas keyed by property name. See module-level
    /// docs on v0.3 BREAKING change from `serde_json::Value`.
    #[serde(default)]
    pub properties: HashMap<String, Schema>,
    /// References to data sources within this database container.
    /// Introduced in API 2025-09-03.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_sources: Option<Vec<DataSourceRef>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DataSourceRef {
    pub id: DataSourceId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
