//! Notion `/v1/databases/*` endpoints — container operations.
//!
//! As of API 2025-09-03, a database is a *container* for one or more
//! data sources. Schema and page operations live on data sources
//! (see [`crate::api::data_source`]); this module handles the
//! container itself.

use crate::api::client::NotionClient;
use crate::api::error::ApiError;
use crate::types::Database;
use crate::validation::DatabaseId;

impl NotionClient {
    /// `GET /v1/databases/{id}`.
    ///
    /// Returns the container, including its `data_sources` array — use
    /// the first entry's id as the parent for page creation.
    pub async fn retrieve_database(&self, id: &DatabaseId) -> Result<Database, ApiError> {
        self.get(&format!("/databases/{id}")).await
    }
}
