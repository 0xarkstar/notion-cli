//! Notion comment object.
//!
//! Wire format (API 2026-03-11):
//!
//! ```json
//! {
//!   "object":"comment",
//!   "id":"<uuid>",
//!   "parent":{"type":"page_id","page_id":"..."},
//!   "discussion_id":"<uuid>",
//!   "created_time":"...","last_edited_time":"...",
//!   "created_by":{"object":"user","id":"..."},
//!   "rich_text":[...]
//! }
//! ```
//!
//! Notion's model is discussion-based, not reply-hierarchy — replies
//! happen by posting new comments to the same `discussion_id`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::common::UserRef;
use crate::types::rich_text::RichText;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Comment {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    /// Parent reference — shape varies: `{"type":"page_id","page_id":"..."}`
    /// or `{"type":"block_id","block_id":"..."}`. Kept as raw JSON
    /// because parent kinds multiply across Notion objects; typed
    /// coverage can wait until an operator needs to filter by parent kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<serde_json::Value>,
    pub discussion_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_edited_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<UserRef>,
    #[serde(default)]
    pub rich_text: Vec<RichText>,
}
