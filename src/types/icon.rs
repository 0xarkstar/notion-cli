//! Notion `icon` and `cover` wire shapes.
//!
//! Used by database, data source, and page objects on both the read
//! and write path. Wire shapes:
//!
//! ```text
//! Emoji icon:    {"type": "emoji", "emoji": "🚀"}
//! External icon: {"type": "external", "external": {"url": "https://..."}}
//! External cover:{"type": "external", "external": {"url": "https://..."}}
//! File (read):   {"type": "file", "file": {"url": "...", "expiry_time": "..."}}
//! ```
//!
//! Covers are image-only (Notion has no emoji-cover form). File
//! icons and covers are server-owned (presigned upload required);
//! v0.3 writes support emoji + external only. Reads accept all
//! three — file form is parsed via the typed variant below.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Notion page / database / data-source icon.
///
/// Use [`Icon::parse_cli`] to construct from a CLI flag value:
/// anything starting with `http://` or `https://` is treated as an
/// external URL, otherwise the value is taken as an emoji literal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Icon {
    Emoji { emoji: String },
    External { external: ExternalFile },
    File { file: FileRef },
}

impl Icon {
    #[must_use]
    pub fn emoji(emoji: impl Into<String>) -> Self {
        Self::Emoji { emoji: emoji.into() }
    }

    #[must_use]
    pub fn external(url: impl Into<String>) -> Self {
        Self::External { external: ExternalFile { url: url.into() } }
    }

    /// Parse a CLI flag value. URL prefixes → external; otherwise emoji.
    #[must_use]
    pub fn parse_cli(value: &str) -> Self {
        if value.starts_with("http://") || value.starts_with("https://") {
            Self::external(value)
        } else {
            Self::emoji(value)
        }
    }
}

/// Notion page / database cover image. Covers are URL-only — no
/// emoji form, unlike [`Icon`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Cover {
    External { external: ExternalFile },
    File { file: FileRef },
}

impl Cover {
    #[must_use]
    pub fn external(url: impl Into<String>) -> Self {
        Self::External { external: ExternalFile { url: url.into() } }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExternalFile {
    pub url: String,
}

/// Server-owned file reference. Typically carries an `expiry_time`
/// for presigned URLs; write paths in v0.3 do not emit this variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FileRef {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry_time: Option<String>,
}
