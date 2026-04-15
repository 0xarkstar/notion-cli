use thiserror::Error;

/// Cap the length of user input echoed back in error messages.
/// Keeps errors readable and prevents log blowup on huge adversarial inputs.
const MAX_ERROR_INPUT_LEN: usize = 64;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid Notion ID ({reason}): {input}")]
    InvalidId { reason: &'static str, input: String },

    #[error("could not extract Notion ID from URL: {0}")]
    InvalidUrl(String),
}

impl Error {
    pub(crate) fn invalid_id(reason: &'static str, input: &str) -> Self {
        let truncated = if input.len() > MAX_ERROR_INPUT_LEN {
            format!("{}…", &input[..MAX_ERROR_INPUT_LEN])
        } else {
            input.to_string()
        };
        Self::InvalidId { reason, input: truncated }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
