//! Generic paginated response wrapper for Notion list endpoints.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PaginatedResponse<T> {
    pub results: Vec<T>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

impl<T> PaginatedResponse<T> {
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        !self.has_more || self.next_cursor.is_none()
    }
}
