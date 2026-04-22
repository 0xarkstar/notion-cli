//! Notion property *schemas* — the wire shape sent to
//! `POST /v1/databases` and `PATCH /v1/data_sources/{id}`.
//!
//! # Schema vs value
//!
//! A property **schema** describes the *shape* of a property (e.g. "a
//! select with options High / Medium / Low"). A property **value** is
//! the *data* stored under that shape (e.g. "High"). The two wire
//! formats share the `type` discriminator, but their bodies differ:
//!
//! ```text
//! Schema  (POST /v1/databases, PATCH /v1/data_sources/{id}):
//!   {"Priority": {"type": "select", "select": {"options": [{"name": "High"}]}}}
//!
//! Value   (POST /v1/pages):
//!   {"Priority": {"type": "select", "select": {"name": "High"}}}
//! ```
//!
//! See [`PropertyValue`](crate::types::property::PropertyValue) for the
//! value side. Sharing a single Rust type is a correctness hazard —
//! the two wire shapes are NOT interchangeable.
//!
//! # Design
//!
//! The outer [`Schema`] wrapper mirrors
//! [`Property`](crate::types::property::Property):
//! `#[serde(untagged)]` with a `Known | Raw` graceful-degradation
//! fallback. This preserves read-time forward-compat when Notion
//! ships new schema variants — see
//! [serde issue #912](https://github.com/serde-rs/serde/issues/912)
//! for why the `tag + other` approach does not work.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::common::{SelectOption, StatusOption};
use crate::validation::DataSourceId;

/// Graceful-degradation wrapper for Notion property schemas.
///
/// Deserialisation tries [`Schema::Known`] first (a strictly-typed
/// `#[serde(tag = "type")]` enum); if the `type` discriminator does
/// not match any known variant, it falls through to [`Schema::Raw`],
/// which preserves the full JSON for read access.
///
/// Write paths should use [`PropertySchema`] directly — sending a
/// [`Schema::Raw`] on a write would produce a payload with an unknown
/// `type` discriminator that Notion rejects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Schema {
    /// A schema whose type this crate version knows.
    Known(PropertySchema),
    /// Unknown / future property schema. Preserved as raw JSON for
    /// read-only access; **cannot be sent on write operations.**
    Raw(serde_json::Value),
}

impl Schema {
    pub fn known(value: PropertySchema) -> Self {
        Self::Known(value)
    }

    pub fn as_known(&self) -> Option<&PropertySchema> {
        match self {
            Self::Known(v) => Some(v),
            Self::Raw(_) => None,
        }
    }

    /// Return the inner [`PropertySchema`] iff this schema is safe to
    /// include in a write request. Returns `None` for [`Schema::Raw`],
    /// which would be rejected by the Notion API.
    pub fn as_writable(&self) -> Option<&PropertySchema> {
        self.as_known()
    }

    pub fn into_writable(self) -> Option<PropertySchema> {
        match self {
            Self::Known(v) => Some(v),
            Self::Raw(_) => None,
        }
    }

    pub fn is_writable(&self) -> bool {
        matches!(self, Self::Known(_))
    }
}

/// Marker payload for property-schema variants whose shape carries no
/// configuration (e.g. `title`, `rich_text`, `date`). Serialises as `{}`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct EmptyConfig {}

/// The 22 Notion property *schema* variants, as of API 2026-03-11.
///
/// Wire-format example (schema side):
///
/// ```json
/// {
///   "Priority": {
///     "type": "select",
///     "select": { "options": [{ "name": "High" }, { "name": "Low" }] }
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PropertySchema {
    Title { title: EmptyConfig },
    RichText { rich_text: EmptyConfig },
    Number { number: NumberConfig },
    Select { select: SelectConfig },
    MultiSelect { multi_select: MultiSelectConfig },
    Status { status: StatusConfig },
    Date { date: EmptyConfig },
    People { people: EmptyConfig },
    Files { files: EmptyConfig },
    Checkbox { checkbox: EmptyConfig },
    Url { url: EmptyConfig },
    Email { email: EmptyConfig },
    PhoneNumber { phone_number: EmptyConfig },
    Formula { formula: FormulaConfig },
    Relation { relation: RelationConfig },
    Rollup { rollup: RollupConfig },
    CreatedTime { created_time: EmptyConfig },
    CreatedBy { created_by: EmptyConfig },
    LastEditedTime { last_edited_time: EmptyConfig },
    LastEditedBy { last_edited_by: EmptyConfig },
    UniqueId { unique_id: UniqueIdConfig },
    Verification { verification: EmptyConfig },
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct NumberConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<NumberFormat>,
}

/// Notion number-format enum. The wire vocabulary is large but stable;
/// unknown formats from future API versions fall through [`Schema::Raw`]
/// at the outer wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum NumberFormat {
    #[default]
    Number,
    NumberWithCommas,
    Percent,
    Dollar,
    CanadianDollar,
    SingaporeDollar,
    Euro,
    Pound,
    Yen,
    Ruble,
    Rupee,
    Won,
    Yuan,
    Real,
    Lira,
    Rupiah,
    Franc,
    HongKongDollar,
    NewZealandDollar,
    Krona,
    NorwegianKrone,
    MexicanPeso,
    Rand,
    NewTaiwanDollar,
    DanishKrone,
    Zloty,
    Baht,
    Forint,
    Koruna,
    Shekel,
    ChileanPeso,
    PhilippinePeso,
    Dirham,
    ColombianPeso,
    Riyal,
    Ringgit,
    Leu,
    ArgentinePeso,
    UruguayanPeso,
    PeruvianSol,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct SelectConfig {
    #[serde(default)]
    pub options: Vec<SelectOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct MultiSelectConfig {
    #[serde(default)]
    pub options: Vec<SelectOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct StatusConfig {
    #[serde(default)]
    pub options: Vec<StatusOption>,
    #[serde(default)]
    pub groups: Vec<StatusGroup>,
}

/// A status-group definition on a `status` property schema. Status
/// groups are read-only server-side (clients cannot create new groups
/// via the API), but they appear in the schema response and must
/// round-trip cleanly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StatusGroup {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default)]
    pub option_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FormulaConfig {
    pub expression: String,
}

/// Configuration for a `relation` property schema. Uses the flattened
/// tagged enum [`RelationKind`] to distinguish single-property (one-way)
/// from dual-property (with backlink) relations.
///
/// The `data_source_id` field replaces `database_id` on API 2025-09-03+
/// — do NOT send `database_id` or relation wiring will silently break
/// when Notion completes the data-source migration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RelationConfig {
    pub data_source_id: DataSourceId,
    #[serde(flatten)]
    pub kind: RelationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RelationKind {
    /// One-way relation — no backlink property is created on the
    /// target data source.
    SingleProperty { single_property: EmptyConfig },
    /// Two-way relation — Notion auto-creates a reciprocal property
    /// on the target data source, named per `synced_property_name`.
    DualProperty { dual_property: DualPropertyConfig },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DualPropertyConfig {
    pub synced_property_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced_property_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RollupConfig {
    pub relation_property_name: String,
    pub rollup_property_name: String,
    pub function: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct UniqueIdConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}
