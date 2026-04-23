//! Notion `/v1/users/*` endpoints — user enumeration.
//!
//! v0.3 ships these as **CLI-only** (no MCP exposure per D9) —
//! workspace user enumeration is a privacy-adjacent surface that
//! nothing in the `BlueNode` bootstrap needed. Revisit v0.4 if a
//! real agent use case emerges.

use url::Url;

use crate::api::client::NotionClient;
use crate::api::error::ApiError;
use crate::api::pagination::PaginatedResponse;
use crate::types::user::User;
use crate::validation::UserId;

/// Client-side options for [`NotionClient::list_users`]. Wire-level
/// filters are absent on the Notion API — bot vs person filtering is
/// done client-side at the CLI layer.
#[derive(Debug, Clone, Default)]
pub struct ListUsersOptions {
    /// Results per page, 1-100. Defaults to Notion's 100 cap when None.
    pub page_size: Option<u8>,
    /// Opaque pagination cursor from a previous response's `next_cursor`.
    pub start_cursor: Option<String>,
}

impl NotionClient {
    /// `GET /v1/users` — one page of results.
    ///
    /// Callers that want the full list should paginate by feeding
    /// `resp.next_cursor` back into `options.start_cursor` until
    /// `has_more == false`.
    ///
    /// # Panics
    ///
    /// Panics if the internal query-builder Url cannot parse
    /// `"http://x/"` — a static string that will always parse.
    pub async fn list_users(
        &self,
        options: &ListUsersOptions,
    ) -> Result<PaginatedResponse<User>, ApiError> {
        let mut path = "/users".to_string();
        let mut qs_parts: Vec<String> = Vec::new();
        // Scope the serializer tight so it drops before `.await` —
        // `url::form_urlencoded::Serializer` is !Send; cross-await
        // holding trips rustc with a cryptic "future cannot be sent
        // between threads" error. See memory/cli-development-patterns.md.
        {
            let mut encoder = Url::parse("http://x/").unwrap();
            let mut pairs = encoder.query_pairs_mut();
            if let Some(size) = options.page_size {
                pairs.append_pair("page_size", &size.to_string());
            }
            if let Some(cursor) = options.start_cursor.as_deref() {
                pairs.append_pair("start_cursor", cursor);
            }
            drop(pairs);
            if let Some(q) = encoder.query() {
                qs_parts.push(q.to_string());
            }
        }
        if !qs_parts.is_empty() {
            path.push('?');
            path.push_str(&qs_parts.join("&"));
        }
        self.get(&path).await
    }

    /// `GET /v1/users/{id}`.
    pub async fn retrieve_user(&self, id: &UserId) -> Result<User, ApiError> {
        self.get(&format!("/users/{id}")).await
    }

    /// `GET /v1/users/me` — retrieve the bot user tied to the current
    /// integration token. Does NOT enumerate workspace users — it
    /// returns only the caller's own identity. Safe to expose over
    /// MCP (v0.4 M4 / D9-exception).
    pub async fn retrieve_me(&self) -> Result<User, ApiError> {
        self.get("/users/me").await
    }
}
