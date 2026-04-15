//! Notion database filter expressions.
//!
//! Used internally by the `api/` layer (Phase 2) to construct queries.
//! The MCP boundary accepts raw `serde_json::Value` for filters rather
//! than this typed enum — agent-oriented tool schemas degrade on the
//! deep `$ref` recursion schemars emits for this type.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Compound filter expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum FilterExpression {
    And { and: Vec<FilterExpression> },
    Or { or: Vec<FilterExpression> },
    Property(PropertyFilter),
}

/// A leaf filter targeting a single property.
///
/// The condition is kept as opaque JSON because its shape is
/// property-type specific (e.g., `{"checkbox": {"equals": true}}`,
/// `{"number": {"greater_than": 5}}`, etc.) — 22 property types × ~8
/// operators each would explode the type surface with little gain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PropertyFilter {
    pub property: String,
    #[serde(flatten)]
    pub condition: serde_json::Value,
}
