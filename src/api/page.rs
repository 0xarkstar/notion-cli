//! Notion `/v1/pages/*` endpoints.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::client::NotionClient;
use crate::api::error::ApiError;
use crate::types::page::Page;
use crate::types::property::PropertyValue;
use crate::validation::{DataSourceId, PageId};

/// Parent reference for a new page.
///
/// Since API 2025-09-03, pages live under a data source (not directly
/// under a database). `PageParent::Page` is used for sub-pages.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PageParent {
    #[serde(rename = "data_source_id")]
    DataSource { data_source_id: DataSourceId },
    #[serde(rename = "page_id")]
    Page { page_id: PageId },
}

/// Request body for `POST /v1/pages`.
///
/// Properties use `HashMap<String, PropertyValue>` directly — not the
/// `Property` wrapper — because the `Raw` fallback has no compatible
/// wire format (Notion rejects unknown `type` discriminators on write).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CreatePageRequest {
    pub parent: PageParent,
    pub properties: HashMap<String, PropertyValue>,
}

/// Request body for `PATCH /v1/pages/{id}`.
#[derive(Debug, Clone, Serialize, Default, JsonSchema)]
pub struct UpdatePageRequest {
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, PropertyValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_trash: Option<bool>,
}

impl NotionClient {
    /// `GET /v1/pages/{id}`.
    pub async fn retrieve_page(&self, id: &PageId) -> Result<Page, ApiError> {
        self.get(&format!("/pages/{id}")).await
    }

    /// `POST /v1/pages`.
    pub async fn create_page(&self, req: &CreatePageRequest) -> Result<Page, ApiError> {
        self.post("/pages", req).await
    }

    /// `PATCH /v1/pages/{id}`.
    pub async fn update_page(
        &self,
        id: &PageId,
        req: &UpdatePageRequest,
    ) -> Result<Page, ApiError> {
        self.patch(&format!("/pages/{id}"), req).await
    }
}
