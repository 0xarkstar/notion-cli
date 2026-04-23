//! API-layer error types.
//!
//! Never include the Authorization header, raw token, or full request
//! URL in any variant's payload. [`scrub_reqwest_err`][super::client::scrub_reqwest_err]
//! handles reqwest errors specifically.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(
        "request unauthorized — verify NOTION_TOKEN is set to a valid Internal Integration Token from https://www.notion.so/my-integrations"
    )]
    Unauthorized,

    #[error(
        "resource not found — the integration may lack access. In Notion UI: open the page/database → ⋯ menu → Connections → add your integration."
    )]
    NotFound,

    #[error("rate limited (retry after {retry_after:?}s)")]
    RateLimited { retry_after: Option<u64> },

    #[error("Notion validation error [{code}]: {message}{hint}",
        hint = validation_hint(code, message)
            .map(|h| format!("\n  → hint: {h}"))
            .unwrap_or_default())]
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

/// Map common Notion `validation_error` signals to a user-actionable hint.
///
/// Returns `None` when no pattern matches — the error stays bare.
/// Hints are one line, imperative, no speculation. Testable via
/// `ApiError::Validation { ... }.to_string()`.
fn validation_hint(code: &str, message: &str) -> Option<&'static str> {
    let msg = message.to_ascii_lowercase();

    // Wiki-type databases don't allow multi-source.
    if msg.contains("can't add data sources to a wiki")
        || msg.contains("data sources to a wiki")
    {
        return Some(
            "Notion wiki databases cannot have additional data sources. Use the existing data source (`notion-cli db get <id>` → `data_sources[0].id`) to add pages instead.",
        );
    }

    // "X is not a property that exists" (create_page / update_page).
    if msg.contains("is not a property that exists") {
        return Some(
            "Property name must exactly match the data source schema. Run `notion-cli ds get <data_source_id>` to list valid property names.",
        );
    }

    // Archived/trashed parent on create.
    if msg.contains("archived")
        && (msg.contains("parent") || msg.contains("cannot"))
    {
        return Some(
            "Parent page/database is archived or in trash. Restore it in Notion UI before writing.",
        );
    }

    // Type mismatch on property value.
    if msg.contains("expected") && msg.contains("got") {
        return Some(
            "Property value type mismatch with schema. Run `notion-cli schema property-value --pretty` for the correct shape per type.",
        );
    }

    // Integration-not-shared case (often surfaces as object_not_found).
    if code == "object_not_found" {
        return Some(
            "Share the target with your integration: Notion UI → ⋯ → Connections → add integration.",
        );
    }

    // Unsupported body shape on update_block.
    if msg.contains("block type")
        && (msg.contains("cannot be updated") || msg.contains("immutable"))
    {
        return Some(
            "This block type cannot be edited after creation. Delete and re-append instead.",
        );
    }

    // Target data source not shared with integration (relation wiring).
    if msg.contains("target data source not shared with integration") {
        return Some(
            "The target data source is not shared with this integration. Open the target's Share menu and add the integration.",
        );
    }

    // Wiki databases cannot parent other databases.
    if msg.contains("cannot create database under wiki") {
        return Some(
            "Wikis cannot parent databases directly. Create a regular page first and add the database there.",
        );
    }

    // Synced block property name collision.
    if msg.contains("property name conflicts with synced content") {
        return Some(
            "A synced block or reference holds a property with this name. Rename the property or remove the synced reference.",
        );
    }

    // Page move restrictions.
    if msg.contains("cannot move page") {
        return Some(
            "Page moves require (a) source is a regular page (not database), (b) integration has edit access to new parent, (c) same workspace as source.",
        );
    }

    None
}

/// Map HTTP-level errors (status code + context) to a user-actionable hint.
///
/// Called from the `ServerError` display path — returns `None` when no
/// pattern matches.
pub fn server_error_hint(status: u16, message: &str) -> Option<&'static str> {
    let msg = message.to_ascii_lowercase();

    // Workspace-level operations require OAuth user token, not integration token.
    if status == 403 && (msg.contains("workspace") || msg.contains("workspace_id")) {
        return Some(
            "Workspace-level operations (like moving databases to workspace root) require an OAuth user token. Integration tokens typically receive 403 here.",
        );
    }

    // filter_properties contains an invalid property ID.
    if (msg.contains("filter_properties") || msg.contains("not found"))
        && (status == 404 || msg.contains("404"))
    {
        return Some(
            "Property ID passed via --properties / filter_properties is invalid. Property IDs are the internal Notion identifiers, not display names. Retrieve the page without --properties first to see valid IDs.",
        );
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wiki_data_source_hint() {
        let e = ApiError::Validation {
            code: "validation_error".into(),
            message: "Can't add data sources to a wiki.".into(),
        };
        let s = e.to_string();
        assert!(s.contains("hint:"), "got: {s}");
        assert!(s.contains("existing data source"), "got: {s}");
    }

    #[test]
    fn missing_property_hint() {
        let e = ApiError::Validation {
            code: "validation_error".into(),
            message: "Foo is not a property that exists.".into(),
        };
        assert!(e.to_string().contains("notion-cli ds get"));
    }

    #[test]
    fn object_not_found_suggests_sharing() {
        let e = ApiError::Validation {
            code: "object_not_found".into(),
            message: "Could not find page.".into(),
        };
        assert!(e.to_string().contains("Connections"));
    }

    #[test]
    fn unknown_code_no_hint() {
        let e = ApiError::Validation {
            code: "validation_error".into(),
            message: "Totally novel failure.".into(),
        };
        let s = e.to_string();
        assert!(!s.contains("hint:"), "should not add hint: {s}");
    }
}
