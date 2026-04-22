//! Notion `/v1/pages/*` endpoints.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::client::NotionClient;
use crate::api::error::ApiError;
use crate::types::icon::{Cover, Icon};
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
///
/// `children` optionally provides the page body at creation time —
/// one-shot page + body in a single API call (preferred over
/// create + append).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CreatePageRequest {
    pub parent: PageParent,
    pub properties: HashMap<String, PropertyValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<crate::types::block::BlockBody>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<Icon>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<Cover>,
}

/// Request body for `PATCH /v1/pages/{id}`.
///
/// `icon` and `cover` are **tristate** via `Option<Option<_>>`:
/// - `None` → field absent in body → leave unchanged
/// - `Some(None)` → emitted as JSON `null` → clear
/// - `Some(Some(v))` → emitted as the value → set
#[derive(Debug, Clone, Serialize, Default, JsonSchema)]
pub struct UpdatePageRequest {
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, PropertyValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_trash: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<Option<Icon>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<Option<Cover>>,
}

/// Where a `page move` call should send the page.
///
/// `PATCH /v1/pages/{id}` explicitly rejects parent mutation — per
/// Notion's docs: "A page's parent cannot be changed" on PATCH.
/// Use the dedicated `POST /v1/pages/{page_id}/move` endpoint
/// introduced 2026-01-15 (D12 smoke test confirmed).
///
/// Target types supported by Notion:
/// - `ToPage(PageId)` — move under a regular page
/// - `ToDataSource(DataSourceId)` — move into a database's data source
///
/// Notion accepts `data_source_id` (not `database_id`) on API
/// 2025-09-03+. Self-moves (same parent) are server-rejected.
#[derive(Debug, Clone)]
pub enum MoveTarget {
    ToPage(PageId),
    ToDataSource(DataSourceId),
}

/// The `parent` block on the move-page request body. Mirrors
/// [`PageParent`] but with the 2026-01-15 move-specific variants.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParentForMove {
    #[serde(rename = "page_id")]
    Page { page_id: PageId },
    #[serde(rename = "data_source_id")]
    DataSource { data_source_id: DataSourceId },
}

impl From<MoveTarget> for ParentForMove {
    fn from(t: MoveTarget) -> Self {
        match t {
            MoveTarget::ToPage(id) => Self::Page { page_id: id },
            MoveTarget::ToDataSource(id) => Self::DataSource { data_source_id: id },
        }
    }
}

/// Request body for `POST /v1/pages/{page_id}/move`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MovePageRequest {
    pub parent: ParentForMove,
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

    /// `POST /v1/pages/{id}/move` — relocate a page to a new parent.
    ///
    /// Notion restrictions to surface in error hints:
    /// - Must be a regular page (not a database).
    /// - The integration must have edit access to the new parent.
    /// - Cross-workspace moves are rejected.
    pub async fn move_page(
        &self,
        id: &PageId,
        target: MoveTarget,
    ) -> Result<Page, ApiError> {
        let req = MovePageRequest { parent: ParentForMove::from(target) };
        self.post(&format!("/pages/{id}/move"), &req).await
    }
}
