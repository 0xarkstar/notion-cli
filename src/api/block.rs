//! Notion `/v1/blocks/*` endpoints.
//!
//! Blocks are the content primitives of Notion pages — paragraphs,
//! headings, lists, code, etc. A page's body is a tree of blocks;
//! children are fetched via pagination.

use schemars::JsonSchema;
use serde::Serialize;

use crate::api::client::NotionClient;
use crate::api::error::ApiError;
use crate::api::pagination::PaginatedResponse;
use crate::types::block::{Block, BlockBody};
use crate::validation::BlockId;

// === Request bodies =======================================================

/// Request body for `PATCH /v1/blocks/{id}/children` (append).
///
/// `children` send only [`BlockBody`] — metadata fields (id,
/// timestamps, etc.) are assigned by Notion.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AppendBlockChildrenRequest {
    pub children: Vec<BlockBody>,
    /// Optional target block ID to append after. Default: end of
    /// parent's children.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<BlockId>,
}

/// Request body for `PATCH /v1/blocks/{id}` (update).
///
/// Send the block-type-specific content to update the block in place.
/// Only the content fields are mutable; structural things (type, id)
/// cannot change. `archived` / `in_trash` toggle deletion.
#[derive(Debug, Clone, Default, Serialize, JsonSchema)]
pub struct UpdateBlockRequest {
    /// Type-specific content to replace. Must match the block's
    /// existing type.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub body: Option<BlockBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_trash: Option<bool>,
}

// === API surface ==========================================================

impl NotionClient {
    /// `GET /v1/blocks/{id}`.
    pub async fn retrieve_block(&self, id: &BlockId) -> Result<Block, ApiError> {
        self.get(&format!("/blocks/{id}")).await
    }

    /// `GET /v1/blocks/{id}/children` — paginated.
    ///
    /// `start_cursor` / `page_size` passed via query string (rare for
    /// this API: most Notion lists use POST-body cursors, but block
    /// children uses GET-query-string cursors). Values are
    /// percent-encoded for robustness against future Notion cursor
    /// formats that may contain reserved URL characters.
    pub async fn list_block_children(
        &self,
        id: &BlockId,
        start_cursor: Option<&str>,
        page_size: Option<u8>,
    ) -> Result<PaginatedResponse<Block>, ApiError> {
        let encoded = {
            let mut qs = url::form_urlencoded::Serializer::new(String::new());
            if let Some(c) = start_cursor {
                qs.append_pair("start_cursor", c);
            }
            if let Some(p) = page_size {
                qs.append_pair("page_size", &p.to_string());
            }
            qs.finish()
        };
        let path = if encoded.is_empty() {
            format!("/blocks/{id}/children")
        } else {
            format!("/blocks/{id}/children?{encoded}")
        };
        self.get(&path).await
    }

    /// `PATCH /v1/blocks/{id}/children` — append new children.
    pub async fn append_block_children(
        &self,
        id: &BlockId,
        req: &AppendBlockChildrenRequest,
    ) -> Result<PaginatedResponse<Block>, ApiError> {
        self.patch(&format!("/blocks/{id}/children"), req).await
    }

    /// `PATCH /v1/blocks/{id}` — update block content / archive state.
    pub async fn update_block(
        &self,
        id: &BlockId,
        req: &UpdateBlockRequest,
    ) -> Result<Block, ApiError> {
        self.patch(&format!("/blocks/{id}"), req).await
    }

    /// `DELETE /v1/blocks/{id}` — archive (soft-delete) the block.
    pub async fn delete_block(&self, id: &BlockId) -> Result<Block, ApiError> {
        self.delete(&format!("/blocks/{id}")).await
    }
}

// === Request-body convenience ============================================

impl AppendBlockChildrenRequest {
    #[must_use]
    pub fn new(children: Vec<BlockBody>) -> Self {
        Self { children, after: None }
    }
}

/// Convenience: deserialize a `Vec<BlockBody>` from a JSON array
/// string — used by the CLI to accept `--children '[{...},{...}]'`.
///
/// # Errors
/// Returns [`serde_json::Error`] if the input is not a valid JSON
/// array of block bodies.
pub fn parse_children(json: &str) -> Result<Vec<BlockBody>, serde_json::Error> {
    serde_json::from_str(json)
}
