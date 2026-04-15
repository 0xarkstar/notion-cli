use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid Notion ID: {reason}")]
    InvalidId { reason: &'static str },

    #[error("could not extract Notion ID from URL: {0}")]
    InvalidUrl(String),
}

pub type Result<T> = std::result::Result<T, Error>;
