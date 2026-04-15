//! Notion data source — the per-table schema container introduced in
//! API 2025-09-03 to support multi-source databases.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::rich_text::RichText;
use crate::validation::{DataSourceId, DatabaseId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DataSource {
    pub id: DataSourceId,
    pub created_time: String,
    pub last_edited_time: String,
    pub name: String,
    #[serde(default)]
    pub description: Vec<RichText>,
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_parent: Option<DatabaseParentRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DatabaseParentRef {
    pub database_id: DatabaseId,
}
