//! Property value roundtrip suite — all 22 variants.
//!
//! Validates:
//! 1. Every known `PropertyValue` variant serialises and deserialises
//!    without data loss (proptest-generated, full 22-variant coverage).
//! 2. Unknown property types fall through to `Property::Raw`.
//! 3. Malformed KNOWN variants also fall through to `Property::Raw` —
//!    this is a documented footgun; callers that need a hard error on
//!    malformed data must deserialize directly to `PropertyValue`,
//!    not `Property`.
//! 4. `RichText` with all three content types (text, mention, equation)
//!    round-trips cleanly.

use notion_cli::types::common::{
    Color, DateValue, RelationRef, SelectOption, StatusOption, UniqueIdValue, UserRef,
};
use notion_cli::types::rich_text::{
    Annotations, EquationContent, RichText, RichTextContent, TextContent,
};
use notion_cli::types::{Property, PropertyValue};
use notion_cli::validation::{PageId, UserId};

use proptest::prelude::*;

// === Strategies ============================================================

fn arb_color() -> impl Strategy<Value = Color> {
    prop_oneof![
        Just(Color::Default),
        Just(Color::Gray),
        Just(Color::Brown),
        Just(Color::Orange),
        Just(Color::Yellow),
        Just(Color::Green),
        Just(Color::Blue),
        Just(Color::Purple),
        Just(Color::Pink),
        Just(Color::Red),
        Just(Color::GrayBackground),
        Just(Color::BrownBackground),
        Just(Color::OrangeBackground),
        Just(Color::YellowBackground),
        Just(Color::GreenBackground),
        Just(Color::BlueBackground),
        Just(Color::PurpleBackground),
        Just(Color::PinkBackground),
        Just(Color::RedBackground),
    ]
}

fn arb_rich_text_text() -> impl Strategy<Value = RichText> {
    ("[a-zA-Z0-9 ]{0,20}", arb_color()).prop_map(|(s, color)| RichText {
        content: RichTextContent::Text {
            text: TextContent { content: s.clone(), link: None },
        },
        annotations: Annotations { color, ..Default::default() },
        plain_text: s,
        href: None,
    })
}

fn arb_rich_text_equation() -> impl Strategy<Value = RichText> {
    "[a-z + 0-9 =]{1,20}".prop_map(|expr| RichText {
        content: RichTextContent::Equation {
            equation: EquationContent { expression: expr.clone() },
        },
        annotations: Annotations::default(),
        plain_text: expr,
        href: None,
    })
}

fn arb_rich_text_mention() -> impl Strategy<Value = RichText> {
    "[a-zA-Z0-9]{1,32}".prop_map(|ref_id| RichText {
        content: RichTextContent::Mention {
            mention: serde_json::json!({"type": "user", "user": {"id": ref_id}}),
        },
        annotations: Annotations::default(),
        plain_text: "@user".into(),
        href: None,
    })
}

fn arb_rich_text() -> impl Strategy<Value = RichText> {
    prop_oneof![
        arb_rich_text_text(),
        arb_rich_text_equation(),
        arb_rich_text_mention(),
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

fn arb_user_ref() -> impl Strategy<Value = UserRef> {
    "[0-9a-f]{32}".prop_map(|hex| UserRef { id: UserId::parse(&hex).unwrap(), object: None })
}

fn arb_relation_ref() -> impl Strategy<Value = RelationRef> {
    "[0-9a-f]{32}".prop_map(|hex| RelationRef { id: PageId::parse(&hex).unwrap() })
}

fn arb_date_value() -> impl Strategy<Value = DateValue> {
    prop_oneof![
        Just(DateValue {
            start: "2026-01-01".into(),
            end: None,
            time_zone: None,
        }),
        Just(DateValue {
            start: "2026-04-15T10:00:00.000Z".into(),
            end: Some("2026-04-15T11:00:00.000Z".into()),
            time_zone: None,
        }),
        Just(DateValue {
            start: "2026-04-15".into(),
            end: Some("2026-04-16".into()),
            time_zone: Some("America/Los_Angeles".into()),
        }),
    ]
}

fn arb_unique_id() -> impl Strategy<Value = UniqueIdValue> {
    (
        proptest::option::of(0_i64..10_000),
        proptest::option::of("[A-Z]{1,4}"),
    )
        .prop_map(|(number, prefix)| UniqueIdValue { number, prefix })
}

fn arb_write_path_property() -> impl Strategy<Value = PropertyValue> {
    prop_oneof![
        prop::collection::vec(arb_rich_text(), 0..3).prop_map(|v| PropertyValue::Title { title: v }),
        prop::collection::vec(arb_rich_text(), 0..3)
            .prop_map(|v| PropertyValue::RichText { rich_text: v }),
        proptest::option::of((-1.0e9f64..1.0e9f64).prop_filter("finite", |n| n.is_finite()))
            .prop_map(|n| PropertyValue::Number { number: n }),
        proptest::option::of(arb_select_option()).prop_map(|s| PropertyValue::Select { select: s }),
        prop::collection::vec(arb_select_option(), 0..3)
            .prop_map(|v| PropertyValue::MultiSelect { multi_select: v }),
        proptest::option::of(arb_status_option()).prop_map(|s| PropertyValue::Status { status: s }),
        proptest::option::of(arb_date_value()).prop_map(|d| PropertyValue::Date { date: d }),
        prop::collection::vec(arb_user_ref(), 0..3).prop_map(|v| PropertyValue::People { people: v }),
        any::<bool>().prop_map(|b| PropertyValue::Checkbox { checkbox: b }),
        "[a-z]{3,10}"
            .prop_map(|u| PropertyValue::Url { url: Some(format!("https://{u}.com")) }),
        "[a-z]{3,10}"
            .prop_map(|u| PropertyValue::Email { email: Some(format!("{u}@example.com")) }),
        "[0-9]{10}".prop_map(|p| PropertyValue::PhoneNumber { phone_number: Some(p) }),
        prop::collection::vec(arb_relation_ref(), 0..3)
            .prop_map(|v| PropertyValue::Relation { relation: v }),
    ]
}

fn arb_read_only_property() -> impl Strategy<Value = PropertyValue> {
    prop_oneof![
        prop::collection::vec(Just(serde_json::json!({"name": "f.png", "type": "external", "external": {"url": "https://x/y"}})), 0..2)
            .prop_map(|v| PropertyValue::Files { files: v }),
        Just(PropertyValue::Formula {
            formula: serde_json::json!({"type": "string", "string": "computed"}),
        }),
        Just(PropertyValue::Rollup {
            rollup: serde_json::json!({"type": "number", "number": 42.0, "function": "sum"}),
        }),
        Just(PropertyValue::CreatedTime {
            created_time: "2026-04-15T10:00:00.000Z".into(),
        }),
        arb_user_ref().prop_map(|u| PropertyValue::CreatedBy { created_by: u }),
        Just(PropertyValue::LastEditedTime {
            last_edited_time: "2026-04-15T11:00:00.000Z".into(),
        }),
        arb_user_ref().prop_map(|u| PropertyValue::LastEditedBy { last_edited_by: u }),
        arb_unique_id().prop_map(|u| PropertyValue::UniqueId { unique_id: u }),
        Just(PropertyValue::Verification {
            verification: serde_json::json!({"state": "verified"}),
        }),
    ]
}

fn arb_any_property() -> impl Strategy<Value = PropertyValue> {
    prop_oneof![
        arb_write_path_property().boxed(),
        arb_read_only_property().boxed(),
    ]
}

// === Roundtrip proptests — all 22 variants ================================

proptest! {
    #[test]
    fn property_value_serde_roundtrip_all_22(value in arb_any_property()) {
        let json = serde_json::to_value(&value).expect("serialise");
        let back: PropertyValue = serde_json::from_value(json).expect("deserialise");
        prop_assert_eq!(value, back);
    }

    #[test]
    fn property_wrapper_roundtrip_known_all_22(value in arb_any_property()) {
        let wrapped = Property::Known(value.clone());
        let json = serde_json::to_value(&wrapped).expect("serialise");
        let back: Property = serde_json::from_value(json).expect("deserialise");
        prop_assert_eq!(Property::Known(value), back);
    }

    #[test]
    fn property_value_tag_is_present_on_wire(value in arb_any_property()) {
        let json = serde_json::to_value(&value).expect("serialise");
        let obj = json.as_object().expect("object");
        prop_assert!(obj.contains_key("type"), "missing 'type' discriminator in {:?}", json);
    }
}

// === RichText: all three content variants ================================

proptest! {
    #[test]
    fn rich_text_all_variants_roundtrip(rt in arb_rich_text()) {
        let json = serde_json::to_value(&rt).expect("serialise");
        let back: RichText = serde_json::from_value(json).expect("deserialise");
        prop_assert_eq!(rt, back);
    }
}

#[test]
fn rich_text_mention_wire_format() {
    let json = serde_json::json!({
        "type": "mention",
        "mention": {"type": "user", "user": {"id": "abcdef0123456789abcdef0123456789"}},
        "annotations": {"bold": false, "italic": false, "strikethrough": false, "underline": false, "code": false, "color": "default"},
        "plain_text": "@someone"
    });
    let rt: RichText = serde_json::from_value(json).unwrap();
    assert!(matches!(rt.content, RichTextContent::Mention { .. }));
    assert_eq!(rt.plain_text, "@someone");
}

#[test]
fn rich_text_equation_wire_format() {
    let json = serde_json::json!({
        "type": "equation",
        "equation": {"expression": "a + b = c"},
        "annotations": {"bold": false, "italic": false, "strikethrough": false, "underline": false, "code": false, "color": "default"},
        "plain_text": "a + b = c"
    });
    let rt: RichText = serde_json::from_value(json).unwrap();
    assert!(matches!(rt.content, RichTextContent::Equation { .. }));
}

// === Property write-safety API ===========================================

#[test]
fn property_as_writable_returns_known() {
    let p = Property::Known(PropertyValue::Checkbox { checkbox: true });
    assert!(p.is_writable());
    assert!(p.as_writable().is_some());
}

#[test]
fn property_as_writable_rejects_raw() {
    let p = Property::Raw(serde_json::json!({"type": "future_type"}));
    assert!(!p.is_writable());
    assert!(p.as_writable().is_none());
    assert!(p.into_writable().is_none());
}

// === Graceful degradation ==================================================

#[test]
fn unknown_property_type_falls_through_to_raw() {
    let json = serde_json::json!({
        "type": "future_property_type_not_yet_invented",
        "future_field": {"nested": [1, 2, 3]}
    });
    let back: Property = serde_json::from_value(json.clone()).unwrap();
    match back {
        Property::Raw(v) => assert_eq!(v, json),
        Property::Known(_) => panic!("expected Raw fallback"),
    }
}

#[test]
fn known_property_prefers_typed_variant() {
    let json = serde_json::json!({
        "type": "checkbox",
        "checkbox": true
    });
    let back: Property = serde_json::from_value(json).unwrap();
    assert!(
        matches!(back, Property::Known(PropertyValue::Checkbox { checkbox: true })),
        "expected Known(Checkbox(true)), got {back:?}",
    );
}

#[test]
fn raw_preserves_full_payload_for_unknown_type() {
    let json = serde_json::json!({
        "type": "alien_type",
        "alien_type": {"contains": "everything", "nested": {"deep": [1, 2, 3]}},
        "sibling_field": "also preserved"
    });
    let back: Property = serde_json::from_value(json.clone()).unwrap();
    if let Property::Raw(v) = back {
        assert_eq!(v, json, "raw payload must be preserved byte-for-byte");
    } else {
        panic!("expected Raw variant");
    }
}

/// Documents a known footgun: `Property` uses untagged fallback, so a
/// KNOWN variant with a malformed inner field (e.g. `checkbox: "not-a-bool"`)
/// silently falls through to `Raw` instead of producing a deserialise
/// error. Callers who need strict validation should deserialize directly
/// into [`PropertyValue`], not [`Property`].
#[test]
fn malformed_known_variant_falls_through_to_raw_silently() {
    let json = serde_json::json!({
        "type": "checkbox",
        "checkbox": "not-a-bool"
    });

    // Via Property wrapper: silently falls through to Raw.
    let via_wrapper: Property = serde_json::from_value(json.clone()).unwrap();
    assert!(
        matches!(via_wrapper, Property::Raw(_)),
        "untagged fallback swallows malformed Known — expected Raw, got {via_wrapper:?}",
    );

    // Via PropertyValue directly: hard error. This is the strict path.
    let via_value = serde_json::from_value::<PropertyValue>(json);
    assert!(
        via_value.is_err(),
        "direct PropertyValue deserialize must fail loudly on malformed input",
    );
}

// === Concrete wire-format fixtures ========================================

#[test]
fn title_wire_format() {
    let json = serde_json::json!({
        "type": "title",
        "title": [{
            "type": "text",
            "text": {"content": "Hello"},
            "annotations": {
                "bold": false, "italic": false, "strikethrough": false,
                "underline": false, "code": false, "color": "default"
            },
            "plain_text": "Hello"
        }]
    });
    let parsed: PropertyValue = serde_json::from_value(json).unwrap();
    assert!(
        matches!(&parsed, PropertyValue::Title { title } if title.len() == 1),
        "got {parsed:?}",
    );
}

#[test]
fn checkbox_wire_format() {
    let parsed: PropertyValue =
        serde_json::from_value(serde_json::json!({"type": "checkbox", "checkbox": true}))
            .unwrap();
    assert!(matches!(parsed, PropertyValue::Checkbox { checkbox: true }));
}

#[test]
fn status_wire_format_with_option() {
    let parsed: PropertyValue = serde_json::from_value(serde_json::json!({
        "type": "status",
        "status": {"id": "s-1", "name": "In Progress", "color": "blue"}
    }))
    .unwrap();
    assert!(matches!(parsed, PropertyValue::Status { status: Some(_) }));
}

#[test]
fn status_wire_format_null() {
    let parsed: PropertyValue = serde_json::from_value(serde_json::json!({
        "type": "status",
        "status": null
    }))
    .unwrap();
    assert!(matches!(parsed, PropertyValue::Status { status: None }));
}

#[test]
fn formula_stays_opaque() {
    let parsed: PropertyValue = serde_json::from_value(serde_json::json!({
        "type": "formula",
        "formula": {"type": "string", "string": "computed"}
    }))
    .unwrap();
    if let PropertyValue::Formula { formula } = parsed {
        assert!(formula.get("type").is_some());
    } else {
        panic!("expected Formula variant");
    }
}

#[test]
fn rollup_stays_opaque() {
    let parsed: PropertyValue = serde_json::from_value(serde_json::json!({
        "type": "rollup",
        "rollup": {"type": "number", "number": 42.0, "function": "sum"}
    }))
    .unwrap();
    assert!(matches!(parsed, PropertyValue::Rollup { .. }));
}
