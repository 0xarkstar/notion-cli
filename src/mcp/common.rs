//! Shared state + helpers used by all three MCP server tiers.
//!
//! The MCP server is split into three separate impl files
//! (`server_ro.rs`, `server_write.rs`, `server_admin.rs`), each
//! wrapping the same [`Inner`] state. Module boundary is the
//! invariant — an admin-only tool added to the wrong file will NOT
//! accidentally leak into a lower-privilege tier, because each tier's
//! `#[tool_router]` impl is declared in its own file.

use std::sync::Arc;

use rmcp::model::{CallToolResult, Content};

use crate::api::NotionClient;
use crate::mcp::audit::AuditLog;

pub struct Inner {
    pub client: NotionClient,
    pub audit: AuditLog,
}

impl Inner {
    #[must_use]
    pub fn arc(client: NotionClient, audit: AuditLog) -> Arc<Self> {
        Arc::new(Self { client, audit })
    }
}

pub(crate) fn to_result(value: &serde_json::Value) -> CallToolResult {
    let text = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
    CallToolResult::success(vec![Content::text(text)])
}
