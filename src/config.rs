//! Runtime configuration types.

use std::fmt;

/// A Notion integration token.
///
/// Wrapped to prevent leakage via `Debug`, `Display`, or error chains.
/// Only [`Self::expose`] returns the raw string — use it solely to
/// construct the `Authorization: Bearer …` header and nowhere else.
#[derive(Clone)]
pub struct NotionToken(String);

impl NotionToken {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// Load from the `NOTION_TOKEN` environment variable.
    ///
    /// # Errors
    /// Returns the underlying [`std::env::VarError`] on missing /
    /// non-UTF8 env var.
    pub fn from_env() -> Result<Self, std::env::VarError> {
        std::env::var("NOTION_TOKEN").map(Self)
    }

    /// Raw token — never log, never display, never include in error
    /// messages. Only for building the Authorization header.
    #[must_use]
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }

    /// First 4 chars of the token (for telemetry / ops diagnostics).
    ///
    /// Long tokens start with a recognisable prefix (`ntn_`, `secret_`).
    /// This is safe to log.
    #[must_use]
    pub fn prefix(&self) -> String {
        self.0.chars().take(4).collect()
    }
}

impl fmt::Debug for NotionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NotionToken({}…)", self.prefix())
    }
}

impl fmt::Display for NotionToken {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Deliberately empty — never print.
        Ok(())
    }
}
