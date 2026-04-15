//! Schema-shape assertions — the Phase 1 kill-switch.
//!
//! DESIGN.md's "schema is the single source of truth" claim depends on
//! schemars producing usable output. These tests assert the shape is
//! sound enough for the `notion schema` introspection subcommand and
//! for reference by hand-authored MCP tool schemas. They do NOT attempt
//! to validate that the emitted schema is agent-friendly at the MCP
//! boundary — per the revised architecture (§2 of the plan), MCP tool
//! schemas are hand-authored, so schemars output is for internal /
//! introspection use only.

use notion_cli::types::{Property, PropertyValue};
use notion_cli::validation::DatabaseId;

use schemars::schema_for;

const WRITE_PATH_VARIANTS: &[&str] = &[
    "title",
    "rich_text",
    "number",
    "select",
    "multi_select",
    "status",
    "date",
    "people",
    "checkbox",
    "url",
    "email",
    "phone_number",
    "relation",
];

const READ_ONLY_VARIANTS: &[&str] = &[
    "files",
    "formula",
    "rollup",
    "created_time",
    "created_by",
    "last_edited_time",
    "last_edited_by",
    "unique_id",
    "verification",
];

#[test]
fn property_value_schema_is_valid_json() {
    let schema = schema_for!(PropertyValue);
    let json = serde_json::to_value(&schema).unwrap();
    assert!(json.is_object(), "schema must serialise to an object");
}

#[test]
fn property_value_schema_covers_all_22_variants() {
    let schema = schema_for!(PropertyValue);
    let rendered = serde_json::to_string(&schema).unwrap();
    for variant in WRITE_PATH_VARIANTS.iter().chain(READ_ONLY_VARIANTS) {
        assert!(
            rendered.contains(&format!("\"{variant}\"")),
            "schema missing variant '{variant}':\n{rendered}",
        );
    }
}

#[test]
fn property_value_schema_uses_type_discriminator() {
    let schema = schema_for!(PropertyValue);
    let rendered = serde_json::to_string(&schema).unwrap();
    // With #[serde(tag = "type")], every oneOf/anyOf branch should
    // constrain "type" via `const`.
    assert!(
        rendered.contains("\"type\""),
        "schema must reference 'type' discriminator:\n{rendered}",
    );
}

#[test]
fn property_wrapper_schema_is_valid_json() {
    let schema = schema_for!(Property);
    let json = serde_json::to_value(&schema).unwrap();
    assert!(json.is_object(), "wrapper schema must serialise to an object");
}

#[test]
fn database_id_schema_is_plain_string() {
    let schema = schema_for!(DatabaseId);
    let json = serde_json::to_value(&schema).unwrap();
    let obj = json.as_object().expect("schema is object");
    let type_field = obj.get("type").expect("has type field");
    assert_eq!(
        type_field.as_str(),
        Some("string"),
        "DatabaseId must serialise as a plain string schema, got: {json}",
    );
}

#[test]
fn all_22_property_variants_are_named_in_module() {
    // Sanity: ensure we have the expected count matching DESIGN.md.
    let total = WRITE_PATH_VARIANTS.len() + READ_ONLY_VARIANTS.len();
    assert_eq!(
        total, 22,
        "expected 22 PropertyValue variants (13 write-path + 9 read-only), got {total}",
    );
}
