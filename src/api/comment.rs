//! Notion `/v1/comments` — CLI-only in v0.3 (D10).
//!
//! Not exposed over MCP. Comment creation is a content-generation
//! surface that agents rarely legitimately drive; moving it to v0.4
//! if real demand emerges would let us add per-tool rate limiting
//! at that time.
//!
//! Wire shapes (API 2026-03-11):
//!
//! ```text
//! GET  /v1/comments?block_id=<id>       — list comments on a block
//! POST /v1/comments                      — body:
//!   {"parent":{"page_id":"..."}, "rich_text":[...]}   // top-level on page
//!   {"discussion_id":"...",      "rich_text":[...]}   // reply in discussion
//! ```

use schemars::JsonSchema;
use serde::Serialize;
use url::Url;

use crate::api::client::NotionClient;
use crate::api::error::ApiError;
use crate::api::pagination::PaginatedResponse;
use crate::types::comment::Comment;
use crate::types::rich_text::RichText;
use crate::validation::{BlockId, PageId};

/// Options for `comments list` — Notion accepts `block_id` + pagination
/// params on `GET /v1/comments`.
#[derive(Debug, Clone)]
pub struct ListCommentsOptions {
    pub block_id: BlockId,
    pub page_size: Option<u8>,
    pub start_cursor: Option<String>,
}

/// Parent reference for top-level-on-page comment creation.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CommentParent {
    pub page_id: PageId,
}

/// Body for `POST /v1/comments`. Either `parent` (top-level on page)
/// or `discussion_id` (reply in discussion) is required — never both.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CreateCommentRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<CommentParent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discussion_id: Option<String>,
    pub rich_text: Vec<RichText>,
}

impl NotionClient {
    pub async fn list_comments(
        &self,
        options: &ListCommentsOptions,
    ) -> Result<PaginatedResponse<Comment>, ApiError> {
        let mut path = "/comments".to_string();
        let mut qs: Option<String> = None;
        // Scope the serializer tight so it drops before `.await` —
        // see memory note on url::form_urlencoded::Serializer !Send.
        {
            let mut encoder = Url::parse("http://x/").unwrap();
            {
                let mut pairs = encoder.query_pairs_mut();
                pairs.append_pair("block_id", options.block_id.as_str());
                if let Some(size) = options.page_size {
                    pairs.append_pair("page_size", &size.to_string());
                }
                if let Some(cursor) = options.start_cursor.as_deref() {
                    pairs.append_pair("start_cursor", cursor);
                }
            }
            if let Some(q) = encoder.query() {
                qs = Some(q.to_string());
            }
        }
        if let Some(q) = qs {
            path.push('?');
            path.push_str(&q);
        }
        self.get(&path).await
    }

    pub async fn create_comment(
        &self,
        req: &CreateCommentRequest,
    ) -> Result<Comment, ApiError> {
        self.post("/comments", req).await
    }
}
