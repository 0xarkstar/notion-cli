//! Notion `/v1/search` endpoint.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::client::NotionClient;
use crate::api::error::ApiError;
use crate::api::pagination::PaginatedResponse;

/// Request body for `POST /v1/search`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct SearchRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u8>,
}

impl NotionClient {
    /// `POST /v1/search`.
    ///
    /// Results are mixed (pages / data sources / databases depending
    /// on filter). Returned as raw JSON values — callers dispatch by
    /// inspecting each result's `object` field.
    pub async fn search(
        &self,
        req: &SearchRequest,
    ) -> Result<PaginatedResponse<serde_json::Value>, ApiError> {
        self.post("/search", req).await
    }
}
