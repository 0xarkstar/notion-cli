//! Property-schema roundtrip suite — all 22 variants (D4 verification).
//!
//! Validates:
//! 1. Every known `PropertySchema` variant serialises + deserialises
//!    without data loss (proptest-generated, full 22-variant coverage).
//! 2. Unknown / future property-schema types fall through to
//!    `Schema::Raw` — forward-compat contract.
//! 3. `Schema::Raw` cannot be recovered as a writable schema.
//! 4. Concrete wire-format fixtures match Notion's documented shapes
//!    (select options, relation dual_property, status groups).

use notion_cli::types::common::{Color, SelectOption, StatusOption};
use notion_cli::types::property_schema::{
    DualPropertyConfig, EmptyConfig, FormulaConfig, MultiSelectConfig, NumberConfig,
    NumberFormat, PropertySchema, RelationConfig, RelationKind, RollupConfig, Schema,
    SelectConfig, StatusConfig, StatusGroup, UniqueIdConfig,
};
use notion_cli::validation::DataSourceId;

use proptest::prelude::*;

// === Strategies ============================================================

fn arb_color() -> impl Strategy<Value = Color> {
    prop_oneof![
        Just(Color::Default),
        Just(Color::Gray),
        Just(Color::Blue),
        Just(Color::Red),
        Just(Color::Green),
        Just(Color::Yellow),
    ]
}

fn arb_select_option() -> impl Strategy<Value = SelectOption> {
    (
        proptest::option::of("[a-z0-9]{1,10}"),
        "[a-zA-Z]{1,15}",
        proptest::option::of(arb_color()),
    )
        .prop_map(|(id, name, color)| SelectOption { id, name, color })
}

fn arb_status_option() -> impl Strategy<Value = StatusOption> {
    ("[a-zA-Z]{1,15}", arb_color())
        .prop_map(|(name, color)| StatusOption { id: None, name, color: Some(color) })
}

fn arb_status_group() -> impl Strategy<Value = StatusGroup> {
    ("[a-zA-Z]{1,15}", prop::collection::vec("[a-z0-9_]{4,12}", 0..3))
        .prop_map(|(name, option_ids)| StatusGroup {
            name,
            id: None,
            color: None,
            option_ids,
        })
}

fn arb_data_source_id() -> impl Strategy<Value = DataSourceId> {
    "[0-9a-f]{32}".prop_map(|hex| DataSourceId::parse(&hex).unwrap())
}

fn arb_number_format() -> impl Strategy<Value = NumberFormat> {
    prop_oneof![
        Just(NumberFormat::Number),
        Just(NumberFormat::Percent),
        Just(NumberFormat::Dollar),
        Just(NumberFormat::Euro),
        Just(NumberFormat::Won),
    ]
}

fn arb_relation_kind() -> impl Strategy<Value = RelationKind> {
    prop_oneof![
        Just(RelationKind::SingleProperty { single_property: EmptyConfig {} }),
        "[a-zA-Z ]{1,15}".prop_map(|name| RelationKind::DualProperty {
            dual_property: DualPropertyConfig {
                synced_property_name: name,
                synced_property_id: None,
            },
        }),
    ]
}

fn arb_property_schema() -> impl Strategy<Value = PropertySchema> {
    prop_oneof![
        Just(PropertySchema::Title { title: EmptyConfig {} }),
        Just(PropertySchema::RichText { rich_text: EmptyConfig {} }),
        proptest::option::of(arb_number_format())
            .prop_map(|format| PropertySchema::Number { number: NumberConfig { format } }),
        prop::collection::vec(arb_select_option(), 0..3)
            .prop_map(|options| PropertySchema::Select { select: SelectConfig { options } }),
        prop::collection::vec(arb_select_option(), 0..3).prop_map(|options| {
            PropertySchema::MultiSelect { multi_select: MultiSelectConfig { options } }
        }),
        (
            prop::collection::vec(arb_status_option(), 0..3),
            prop::collection::vec(arb_status_group(), 0..2),
        )
            .prop_map(|(options, groups)| PropertySchema::Status {
                status: StatusConfig { options, groups },
            }),
        Just(PropertySchema::Date { date: EmptyConfig {} }),
        Just(PropertySchema::People { people: EmptyConfig {} }),
        Just(PropertySchema::Files { files: EmptyConfig {} }),
        Just(PropertySchema::Checkbox { checkbox: EmptyConfig {} }),
        Just(PropertySchema::Url { url: EmptyConfig {} }),
        Just(PropertySchema::Email { email: EmptyConfig {} }),
        Just(PropertySchema::PhoneNumber { phone_number: EmptyConfig {} }),
        "[a-zA-Z0-9 +*()]{1,20}".prop_map(|expr| PropertySchema::Formula {
            formula: FormulaConfig { expression: expr },
        }),
        (arb_data_source_id(), arb_relation_kind()).prop_map(|(ds, kind)| {
            PropertySchema::Relation {
                relation: RelationConfig { data_source_id: ds, kind },
            }
        }),
        (
            "[a-zA-Z]{1,10}",
            "[a-zA-Z]{1,10}",
            "[a-z_]{3,10}",
        )
            .prop_map(|(rel, roll, func)| PropertySchema::Rollup {
                rollup: RollupConfig {
                    relation_property_name: rel,
                    rollup_property_name: roll,
                    function: func,
                },
            }),
        Just(PropertySchema::CreatedTime { created_time: EmptyConfig {} }),
        Just(PropertySchema::CreatedBy { created_by: EmptyConfig {} }),
        Just(PropertySchema::LastEditedTime { last_edited_time: EmptyConfig {} }),
        Just(PropertySchema::LastEditedBy { last_edited_by: EmptyConfig {} }),
        proptest::option::of("[A-Z]{1,4}").prop_map(|prefix| PropertySchema::UniqueId {
            unique_id: UniqueIdConfig { prefix },
        }),
        Just(PropertySchema::Verification { verification: EmptyConfig {} }),
    ]
}

// === Roundtrip proptests — all 22 variants ================================

proptest! {
    #[test]
    fn property_schema_serde_roundtrip_all_22(schema in arb_property_schema()) {
        let json = serde_json::to_value(&schema).expect("serialise");
        let back: PropertySchema = serde_json::from_value(json).expect("deserialise");
        prop_assert_eq!(schema, back);
    }

    #[test]
    fn schema_wrapper_roundtrip_known_all_22(schema in arb_property_schema()) {
        let wrapped = Schema::Known(schema.clone());
        let json = serde_json::to_value(&wrapped).expect("serialise");
        let back: Schema = serde_json::from_value(json).expect("deserialise");
        prop_assert_eq!(Schema::Known(schema), back);
    }

    #[test]
    fn property_schema_tag_is_present_on_wire(schema in arb_property_schema()) {
        let json = serde_json::to_value(&schema).expect("serialise");
        let obj = json.as_object().expect("object");
        prop_assert!(
            obj.contains_key("type"),
            "missing 'type' discriminator in {:?}",
            json,
        );
    }
}

// === Graceful degradation =================================================

#[test]
fn unknown_schema_type_falls_through_to_raw() {
    let json = serde_json::json!({
        "type": "future_schema_kind_not_invented_yet",
        "future_schema_kind_not_invented_yet": {"some": "config"}
    });
    let back: Schema = serde_json::from_value(json.clone()).unwrap();
    match back {
        Schema::Raw(v) => assert_eq!(v, json),
        Schema::Known(_) => panic!("expected Raw fallback"),
    }
}

#[test]
fn schema_raw_is_not_writable() {
    let raw = Schema::Raw(serde_json::json!({"type": "alien"}));
    assert!(!raw.is_writable());
    assert!(raw.as_writable().is_none());
    assert!(raw.into_writable().is_none());
}

#[test]
fn schema_known_is_writable() {
    let known = Schema::Known(PropertySchema::Title { title: EmptyConfig {} });
    assert!(known.is_writable());
    assert!(known.as_writable().is_some());
}

// === Concrete wire-format fixtures ========================================

#[test]
fn select_schema_wire_format_matches_docs() {
    // Notion docs example for `POST /v1/databases` schema.
    let json = serde_json::json!({
        "type": "select",
        "select": {
            "options": [
                {"name": "High"},
                {"name": "Medium"},
                {"name": "Low"}
            ]
        }
    });
    let parsed: PropertySchema = serde_json::from_value(json).unwrap();
    match parsed {
        PropertySchema::Select { select } => {
            assert_eq!(select.options.len(), 3);
            assert_eq!(select.options[0].name, "High");
        }
        other => panic!("expected Select, got {other:?}"),
    }
}

#[test]
fn title_schema_wire_format_is_empty_body() {
    let json = serde_json::json!({"type": "title", "title": {}});
    let parsed: PropertySchema = serde_json::from_value(json.clone()).unwrap();
    assert!(matches!(parsed, PropertySchema::Title { .. }));

    // Round-trip emits the empty body.
    let back = serde_json::to_value(&parsed).unwrap();
    assert_eq!(back, json);
}

#[test]
fn relation_schema_dual_property_wire_format() {
    // The canonical BlueNode-bootstrap dual-property relation shape
    // — this is the #1 error-prone hot spot per the audit.
    let json = serde_json::json!({
        "type": "relation",
        "relation": {
            "data_source_id": "abcdef0123456789abcdef0123456789",
            "type": "dual_property",
            "dual_property": {
                "synced_property_name": "Backlink"
            }
        }
    });
    let parsed: PropertySchema = serde_json::from_value(json).unwrap();
    match parsed {
        PropertySchema::Relation { relation } => match relation.kind {
            RelationKind::DualProperty { dual_property } => {
                assert_eq!(dual_property.synced_property_name, "Backlink");
            }
            RelationKind::SingleProperty { .. } => {
                panic!("expected DualProperty");
            }
        },
        other => panic!("expected Relation, got {other:?}"),
    }
}

#[test]
fn relation_schema_single_property_wire_format() {
    let json = serde_json::json!({
        "type": "relation",
        "relation": {
            "data_source_id": "abcdef0123456789abcdef0123456789",
            "type": "single_property",
            "single_property": {}
        }
    });
    let parsed: PropertySchema = serde_json::from_value(json).unwrap();
    match parsed {
        PropertySchema::Relation { relation } => {
            assert!(matches!(relation.kind, RelationKind::SingleProperty { .. }));
        }
        other => panic!("expected Relation, got {other:?}"),
    }
}

#[test]
fn status_schema_with_groups_wire_format() {
    let json = serde_json::json!({
        "type": "status",
        "status": {
            "options": [
                {"name": "Todo", "color": "default"},
                {"name": "Doing", "color": "blue"},
                {"name": "Done", "color": "green"}
            ],
            "groups": [
                {"name": "To-do", "option_ids": ["opt-1"]},
                {"name": "In progress", "option_ids": ["opt-2"]},
                {"name": "Complete", "option_ids": ["opt-3"]}
            ]
        }
    });
    let parsed: PropertySchema = serde_json::from_value(json).unwrap();
    match parsed {
        PropertySchema::Status { status } => {
            assert_eq!(status.options.len(), 3);
            assert_eq!(status.groups.len(), 3);
            assert_eq!(status.groups[0].name, "To-do");
        }
        other => panic!("expected Status, got {other:?}"),
    }
}

#[test]
fn number_schema_with_format() {
    let json = serde_json::json!({
        "type": "number",
        "number": {"format": "dollar"}
    });
    let parsed: PropertySchema = serde_json::from_value(json).unwrap();
    match parsed {
        PropertySchema::Number { number } => {
            assert_eq!(number.format, Some(NumberFormat::Dollar));
        }
        other => panic!("expected Number, got {other:?}"),
    }
}

#[test]
fn number_schema_without_format() {
    let json = serde_json::json!({"type": "number", "number": {}});
    let parsed: PropertySchema = serde_json::from_value(json).unwrap();
    match parsed {
        PropertySchema::Number { number } => assert_eq!(number.format, None),
        other => panic!("expected Number, got {other:?}"),
    }
}

#[test]
fn unique_id_schema_with_prefix() {
    let json = serde_json::json!({
        "type": "unique_id",
        "unique_id": {"prefix": "TASK"}
    });
    let parsed: PropertySchema = serde_json::from_value(json).unwrap();
    match parsed {
        PropertySchema::UniqueId { unique_id } => {
            assert_eq!(unique_id.prefix.as_deref(), Some("TASK"));
        }
        other => panic!("expected UniqueId, got {other:?}"),
    }
}

#[test]
fn formula_schema_expression() {
    let json = serde_json::json!({
        "type": "formula",
        "formula": {"expression": "prop(\"Price\") * 1.1"}
    });
    let parsed: PropertySchema = serde_json::from_value(json).unwrap();
    match parsed {
        PropertySchema::Formula { formula } => {
            assert!(formula.expression.contains("Price"));
        }
        other => panic!("expected Formula, got {other:?}"),
    }
}

#[test]
fn rollup_schema_full_shape() {
    let json = serde_json::json!({
        "type": "rollup",
        "rollup": {
            "relation_property_name": "Projects",
            "rollup_property_name": "Budget",
            "function": "sum"
        }
    });
    let parsed: PropertySchema = serde_json::from_value(json).unwrap();
    match parsed {
        PropertySchema::Rollup { rollup } => {
            assert_eq!(rollup.relation_property_name, "Projects");
            assert_eq!(rollup.rollup_property_name, "Budget");
            assert_eq!(rollup.function, "sum");
        }
        other => panic!("expected Rollup, got {other:?}"),
    }
}

// === v0.2 forward-compat =================================================

/// A v0.2 consumer who wrote a `properties` HashMap as
/// `HashMap<String, serde_json::Value>` can still deserialise v0.3
/// responses into `HashMap<String, Schema>` — the Raw fallback
/// catches any variant this crate version does not model.
#[test]
fn v02_era_unrecognised_wrapper_falls_through_raw() {
    // Simulate a hypothetical future Notion schema the crate doesn't
    // know about.
    let json = serde_json::json!({
        "type": "ai_column",
        "ai_column": {
            "prompt": "summarise the page",
            "model": "claude-4"
        }
    });
    let back: Schema = serde_json::from_value(json.clone()).unwrap();
    match back {
        Schema::Raw(v) => assert_eq!(v, json, "Raw payload must round-trip byte-for-byte"),
        Schema::Known(_) => panic!("expected Raw"),
    }
}
