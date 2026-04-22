//! Notion `/v1/databases/*` endpoints — container operations.
//!
//! As of API 2025-09-03, a database is a *container* for one or more
//! data sources. Schema and page operations live on data sources
//! (see [`crate::api::data_source`]); this module handles the
//! container itself.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::client::NotionClient;
use crate::api::error::ApiError;
use crate::types::icon::{Cover, Icon};
use crate::types::property_schema::PropertySchema;
use crate::types::rich_text::RichText;
use crate::types::Database;
use crate::validation::{DatabaseId, PageId};

// === Requests =============================================================

/// Parent reference for a new database. v0.3 supports page parents
/// only (D8): workspace-parented databases require OAuth user tokens
/// that integration tokens lack, so exposing that variant would
/// produce opaque 400s. Add when OAuth support lands in v0.4+.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CreateDatabaseParent {
    #[serde(rename = "page_id")]
    Page { page_id: PageId },
}

impl CreateDatabaseParent {
    #[must_use]
    pub fn page(id: PageId) -> Self {
        Self::Page { page_id: id }
    }
}

/// The `initial_data_source` body block required by `POST /v1/databases`
/// on Notion API 2025-09-03+.
///
/// Notion creates an implicit first data source inside the new
/// container; this struct seeds its property schemas.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InitialDataSource {
    pub properties: HashMap<String, PropertySchema>,
}

/// Request body for `POST /v1/databases`.
///
/// `properties` are strictly-typed [`PropertySchema`] — the write
/// path rejects `Schema::Raw` (see D4). Validate locally via
/// [`CreateDatabaseRequest::validate_local`] before sending.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateDatabaseRequest {
    pub parent: CreateDatabaseParent,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub title: Vec<RichText>,
    pub initial_data_source: InitialDataSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<Icon>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<Cover>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_inline: Option<bool>,
}

impl CreateDatabaseRequest {
    /// Validate structural constraints per D8:
    /// - at least one Title-typed property
    /// - non-empty properties map
    pub fn validate_local(&self) -> Result<(), String> {
        let props = &self.initial_data_source.properties;
        if props.is_empty() {
            return Err(
                "initial_data_source.properties must not be empty".into(),
            );
        }
        let has_title = props
            .values()
            .any(|schema| matches!(schema, PropertySchema::Title { .. }));
        if !has_title {
            return Err(
                "initial_data_source.properties must include one Title-typed property"
                    .into(),
            );
        }
        Ok(())
    }
}

// === API surface ==========================================================

impl NotionClient {
    /// `GET /v1/databases/{id}`.
    ///
    /// Returns the container, including its `data_sources` array — use
    /// the first entry's id as the parent for page creation.
    pub async fn retrieve_database(&self, id: &DatabaseId) -> Result<Database, ApiError> {
        self.get(&format!("/databases/{id}")).await
    }

    /// `POST /v1/databases` — create a new database container.
    ///
    /// Returns the `Database` object (including the auto-created
    /// initial data source reference). The caller is responsible for
    /// having called [`CreateDatabaseRequest::validate_local`] first.
    pub async fn create_database(
        &self,
        req: &CreateDatabaseRequest,
    ) -> Result<Database, ApiError> {
        self.post("/databases", req).await
    }
}
