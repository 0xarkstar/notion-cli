//! API-layer error types.
//!
//! Never include the Authorization header, raw token, or full request
//! URL in any variant's payload. [`scrub_reqwest_err`][super::client::scrub_reqwest_err]
//! handles reqwest errors specifically.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("request unauthorized — check NOTION_TOKEN")]
    Unauthorized,

    #[error("resource not found")]
    NotFound,

    #[error("rate limited (retry after {retry_after:?}s)")]
    RateLimited { retry_after: Option<u64> },

    #[error("Notion validation error [{code}]: {message}")]
    Validation { code: String, message: String },

    #[error("Notion server error (HTTP {status}): {message}")]
    ServerError { status: u16, message: String },

    #[error("response body exceeded cap of {limit_bytes} bytes")]
    BodyTooLarge { limit_bytes: usize },

    #[error("malformed JSON response: {0}")]
    Json(#[from] serde_json::Error),

    #[error("network error ({kind}): {message}")]
    Network { kind: &'static str, message: String },
}

impl ApiError {
    pub(crate) fn network(kind: &'static str, message: impl Into<String>) -> Self {
        Self::Network { kind, message: message.into() }
    }
}
