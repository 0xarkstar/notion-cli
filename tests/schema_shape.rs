//! Schema-shape assertions — the Phase 1 kill-switch.
//!
//! These structurally traverse `schemars` output to verify that the
//! tagged-enum discriminator and every variant name are actually
//! present, rather than relying on substring matches that could be
//! satisfied by description text or unrelated `$ref` entries.

use std::collections::BTreeSet;

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

/// Walk a `PropertyValue` schema's `oneOf` array and extract the
/// `type.const` discriminator from each branch — the structural shape
/// schemars emits for `#[serde(tag = "type")]` enums.
fn extract_tagged_variants(schema_json: &serde_json::Value) -> BTreeSet<String> {
    let defs = schema_json
        .get("$defs")
        .and_then(|v| v.as_object());

    // schema_for!(PropertyValue) puts the oneOf at the root
    // schema_for!(Property) delegates to $defs/PropertyValue
    let pv_schema = if let Some(defs) = defs {
        defs.get("PropertyValue").unwrap_or(schema_json)
    } else {
        schema_json
    };

    let variants = pv_schema
        .get("oneOf")
        .or_else(|| pv_schema.get("anyOf"))
        .and_then(|v| v.as_array())
        .expect("tagged enum schema must have oneOf or anyOf");

    variants
        .iter()
        .filter_map(|branch| {
            branch
                .pointer("/properties/type/const")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .collect()
}

#[test]
fn property_value_schema_is_valid_json() {
    let schema = schema_for!(PropertyValue);
    let json = serde_json::to_value(&schema).unwrap();
    assert!(json.is_object(), "schema must serialise to an object");
}

#[test]
fn property_value_schema_lists_all_22_variants_structurally() {
    let schema = schema_for!(PropertyValue);
    let json = serde_json::to_value(&schema).unwrap();
    let actual = extract_tagged_variants(&json);

    let expected: BTreeSet<String> = WRITE_PATH_VARIANTS
        .iter()
        .chain(READ_ONLY_VARIANTS)
        .map(|s| (*s).to_string())
        .collect();

    assert_eq!(
        actual, expected,
        "schema variants mismatch\nexpected: {expected:?}\nactual:   {actual:?}",
    );
}

#[test]
fn property_value_schema_uses_type_discriminator() {
    let schema = schema_for!(PropertyValue);
    let json = serde_json::to_value(&schema).unwrap();
    let variants = json
        .get("oneOf")
        .or_else(|| json.get("anyOf"))
        .and_then(|v| v.as_array())
        .expect("tagged enum must have oneOf/anyOf");
    for branch in variants {
        assert!(
            branch.pointer("/properties/type/const").is_some(),
            "branch missing type.const discriminator: {branch}",
        );
        let required = branch
            .get("required")
            .and_then(|v| v.as_array())
            .expect("each branch requires type + payload");
        assert!(
            required.iter().any(|v| v == "type"),
            "type must be in required[] on every branch: {branch}",
        );
    }
}

#[test]
fn property_wrapper_schema_is_valid_json() {
    let schema = schema_for!(Property);
    let json = serde_json::to_value(&schema).unwrap();
    assert!(json.is_object(), "wrapper schema must serialise to an object");
}

#[test]
fn property_wrapper_delegates_to_property_value() {
    let schema = schema_for!(Property);
    let json = serde_json::to_value(&schema).unwrap();
    // The Known branch should reference #/$defs/PropertyValue;
    // structurally we require the underlying 22 variants to be
    // reachable via traversal.
    let actual = extract_tagged_variants(&json);
    let expected: BTreeSet<String> = WRITE_PATH_VARIANTS
        .iter()
        .chain(READ_ONLY_VARIANTS)
        .map(|s| (*s).to_string())
        .collect();
    assert_eq!(actual, expected);
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
    let total = WRITE_PATH_VARIANTS.len() + READ_ONLY_VARIANTS.len();
    assert_eq!(
        total, 22,
        "expected 22 PropertyValue variants (13 write-path + 9 read-only), got {total}",
    );
}
