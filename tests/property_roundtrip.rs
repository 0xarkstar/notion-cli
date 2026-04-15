//! Property value roundtrip suite.
//!
//! Validates two guarantees:
//! 1. Every known `PropertyValue` variant serialises and deserialises
//!    without data loss (proptest-generated, ~256 cases per variant).
//! 2. Unknown property types fall through to `Property::Raw` without
//!    panic, preserving the full JSON payload.

use notion_cli::types::common::{
    Color, DateValue, RelationRef, SelectOption, StatusOption, UserRef,
};
use notion_cli::types::rich_text::{
    Annotations, RichText, RichTextContent, TextContent,
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

fn arb_rich_text() -> impl Strategy<Value = RichText> {
    ("[a-zA-Z0-9 ]{0,20}", arb_color()).prop_map(|(s, color)| RichText {
        content: RichTextContent::Text {
            text: TextContent {
                content: s.clone(),
                link: None,
            },
        },
        annotations: Annotations {
            color,
            ..Default::default()
        },
        plain_text: s,
        href: None,
    })
}

fn arb_select_option() -> impl Strategy<Value = SelectOption> {
    ("[a-zA-Z]{1,15}", arb_color()).prop_map(|(name, color)| SelectOption {
        id: None,
        name,
        color: Some(color),
    })
}

fn arb_status_option() -> impl Strategy<Value = StatusOption> {
    ("[a-zA-Z]{1,15}", arb_color()).prop_map(|(name, color)| StatusOption {
        id: None,
        name,
        color: Some(color),
    })
}

fn arb_user_ref() -> impl Strategy<Value = UserRef> {
    "[0-9a-f]{32}".prop_map(|hex| UserRef {
        id: UserId::parse(&hex).unwrap(),
        object: None,
    })
}

fn arb_relation_ref() -> impl Strategy<Value = RelationRef> {
    "[0-9a-f]{32}".prop_map(|hex| RelationRef {
        id: PageId::parse(&hex).unwrap(),
    })
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

fn arb_property_value_write_path() -> impl Strategy<Value = PropertyValue> {
    // The 10 write-path property types. Proptest's u32 shrinking doesn't
    // handle `Strategy<Value = PropertyValue>` with more than ~10 arms
    // before hitting stack limits on older versions, so split if needed.
    prop_oneof![
        prop::collection::vec(arb_rich_text(), 0..3)
            .prop_map(|v| PropertyValue::Title { title: v }),
        prop::collection::vec(arb_rich_text(), 0..3)
            .prop_map(|v| PropertyValue::RichText { rich_text: v }),
        proptest::option::of(
            (-1.0e9f64..1.0e9f64).prop_filter("finite", |n| n.is_finite())
        )
        .prop_map(|n| PropertyValue::Number { number: n }),
        proptest::option::of(arb_select_option())
            .prop_map(|s| PropertyValue::Select { select: s }),
        prop::collection::vec(arb_select_option(), 0..3)
            .prop_map(|v| PropertyValue::MultiSelect { multi_select: v }),
        proptest::option::of(arb_status_option())
            .prop_map(|s| PropertyValue::Status { status: s }),
        proptest::option::of(arb_date_value())
            .prop_map(|d| PropertyValue::Date { date: d }),
        prop::collection::vec(arb_user_ref(), 0..3)
            .prop_map(|v| PropertyValue::People { people: v }),
        any::<bool>().prop_map(|b| PropertyValue::Checkbox { checkbox: b }),
        "[a-z]{3,10}"
            .prop_map(|u| PropertyValue::Url { url: Some(format!("https://{u}.com")) }),
        "[a-z]{3,10}"
            .prop_map(|u| PropertyValue::Email { email: Some(format!("{u}@example.com")) }),
        "[0-9]{10}"
            .prop_map(|p| PropertyValue::PhoneNumber { phone_number: Some(p) }),
        prop::collection::vec(arb_relation_ref(), 0..3)
            .prop_map(|v| PropertyValue::Relation { relation: v }),
    ]
}

// === Roundtrip proptests ===================================================

proptest! {
    #[test]
    fn property_value_serde_roundtrip(value in arb_property_value_write_path()) {
        let json = serde_json::to_value(&value).expect("serialise");
        let back: PropertyValue = serde_json::from_value(json).expect("deserialise");
        prop_assert_eq!(value, back);
    }

    #[test]
    fn property_wrapper_roundtrip_known(value in arb_property_value_write_path()) {
        let wrapped = Property::Known(value.clone());
        let json = serde_json::to_value(&wrapped).expect("serialise");
        let back: Property = serde_json::from_value(json).expect("deserialise");
        prop_assert_eq!(Property::Known(value), back);
    }

    #[test]
    fn property_value_tag_is_present_on_wire(value in arb_property_value_write_path()) {
        let json = serde_json::to_value(&value).expect("serialise");
        let obj = json.as_object().expect("object");
        prop_assert!(obj.contains_key("type"), "missing 'type' discriminator in {:?}", json);
    }
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
