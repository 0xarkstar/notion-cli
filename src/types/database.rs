//! Notion database object.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::rich_text::RichText;
use crate::validation::{DataSourceId, DatabaseId};

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
    /// Raw property schemas — kept as JSON for Phase 1. A typed
    /// `PropertySchema` enum is deferred to v0.2.
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
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
