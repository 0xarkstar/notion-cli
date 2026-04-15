//! CLI error type with structured exit codes.

use thiserror::Error;

use crate::api::ApiError;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Api(#[from] ApiError),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("usage error: {0}")]
    Usage(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl CliError {
    /// Map to a sysexits-style exit code. Stable for downstream
    /// tooling; keep values unchanged across versions.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Api(e) => match e {
                ApiError::Unauthorized => 10,
                ApiError::Validation { .. } => 2,
                ApiError::RateLimited { .. } => 4,
                ApiError::BodyTooLarge { .. }
                | ApiError::NotFound
                | ApiError::ServerError { .. }
                | ApiError::Network { .. } => 3,
                ApiError::Json(_) => 65,
            },
            Self::Validation(_) => 2,
            Self::Config(_) => 10,
            Self::Usage(_) => 64,
            Self::Io(_) => 74,
            Self::Json(_) => 65,
        }
    }
}
