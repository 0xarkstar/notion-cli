//! CLI output formatting.
//!
//! Default mode wraps Notion-origin payloads in an **untrusted
//! envelope**:
//!
//! ```json
//! {
//!   "source": "notion",
//!   "trust": "untrusted",
//!   "api_version": "2026-03-11",
//!   "content": { ... }
//! }
//! ```
//!
//! Agents consuming this output should treat `content` as untrusted
//! data — not as instructions. See the prior DESIGN.md's deleted
//! "sanitization" section for context: sanitising LLM-injection
//! patterns from natural language is unsoundable, so we demarcate
//! instead of strip.
//!
//! `--raw` skips the envelope for clean piping into other tools.

use crate::api::version::NOTION_API_VERSION;

pub struct OutputOptions {
    pub raw: bool,
    pub pretty: bool,
}

/// Wrap an already-serialised value in the untrusted envelope.
#[must_use]
pub fn wrap_untrusted(content: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "source": "notion",
        "trust": "untrusted",
        "api_version": NOTION_API_VERSION,
        "content": content,
    })
}

/// Print a value to stdout using the given options.
///
/// # Errors
/// Returns [`serde_json::Error`] if serialisation fails.
pub fn emit<T: serde::Serialize>(
    options: &OutputOptions,
    value: &T,
) -> Result<(), serde_json::Error> {
    let raw_json = serde_json::to_value(value)?;
    let final_json = if options.raw {
        raw_json
    } else {
        wrap_untrusted(&raw_json)
    };
    if options.pretty {
        println!("{}", serde_json::to_string_pretty(&final_json)?);
    } else {
        println!("{}", serde_json::to_string(&final_json)?);
    }
    Ok(())
}
