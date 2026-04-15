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
use crate::types::page::Page;
use crate::types::property::PropertyValue;
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
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CreateDataSourceRequest {
    pub parent: CreateDataSourceParent,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub title: Vec<RichText>,
    pub properties: serde_json::Value,
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
