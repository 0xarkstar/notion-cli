//! Notion page object.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::property::Property;
use crate::validation::PageId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Page {
    pub id: PageId,
    pub created_time: String,
    pub last_edited_time: String,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub in_trash: bool,
    pub url: String,
    pub properties: HashMap<String, Property>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<serde_json::Value>,
}
