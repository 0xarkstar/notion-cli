//! Notion property values.
//!
//! # Design
//!
//! The outer [`Property`] wrapper is `#[serde(untagged)]`. Deserialisation
//! tries [`Property::Known`] first (a strictly-typed
//! `#[serde(tag = "type")]` enum); if the `type` discriminator does not
//! match any known variant, it falls through to [`Property::Raw`], which
//! preserves the full JSON for read access.
//!
//! This pattern replaces the `#[serde(tag = "type")] + #[serde(other)]`
//! approach suggested in DESIGN.md, which does not work — see
//! [serde issue #912](https://github.com/serde-rs/serde/issues/912).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::common::{
    DateValue, RelationRef, SelectOption, StatusOption, UniqueIdValue, UserRef,
};
use crate::types::rich_text::RichText;

/// Graceful-degradation wrapper for Notion property values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Property {
    /// A property value whose type this crate version knows.
    Known(PropertyValue),
    /// Unknown / future property type. Preserved as raw JSON for
    /// read-only access; cannot be sent on write operations.
    Raw(serde_json::Value),
}

impl Property {
    pub fn known(value: PropertyValue) -> Self {
        Self::Known(value)
    }

    pub fn as_known(&self) -> Option<&PropertyValue> {
        match self {
            Self::Known(v) => Some(v),
            Self::Raw(_) => None,
        }
    }
}

/// The 22 Notion property value variants, as of API 2026-03-11.
///
/// Write-path variants (round-trip safe, used in create/update requests):
/// `Title`, `RichText`, `Number`, `Select`, `MultiSelect`, `Status`,
/// `Date`, `People`, `Checkbox`, `Url`, `Email`, `PhoneNumber`,
/// `Relation`.
///
/// Read-only variants (API does not accept these in writes):
/// `Formula`, `Rollup`, `CreatedTime`, `CreatedBy`, `LastEditedTime`,
/// `LastEditedBy`, `UniqueId`, `Verification`, `Files` (writable but
/// requires presigned uploads — out of scope for v0.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PropertyValue {
    Title { title: Vec<RichText> },
    RichText { rich_text: Vec<RichText> },
    Number { number: Option<f64> },
    Select { select: Option<SelectOption> },
    MultiSelect { multi_select: Vec<SelectOption> },
    Status { status: Option<StatusOption> },
    Date { date: Option<DateValue> },
    People { people: Vec<UserRef> },
    Files { files: Vec<serde_json::Value> },
    Checkbox { checkbox: bool },
    Url { url: Option<String> },
    Email { email: Option<String> },
    PhoneNumber { phone_number: Option<String> },
    Formula { formula: serde_json::Value },
    Relation { relation: Vec<RelationRef> },
    Rollup { rollup: serde_json::Value },
    CreatedTime { created_time: String },
    CreatedBy { created_by: UserRef },
    LastEditedTime { last_edited_time: String },
    LastEditedBy { last_edited_by: UserRef },
    UniqueId { unique_id: UniqueIdValue },
    Verification { verification: serde_json::Value },
}
