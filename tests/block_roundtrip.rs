//! Block roundtrip + graceful-degradation tests (mirrors the
//! `property_roundtrip` suite).

use notion_cli::types::block::{
    Block, BlockBody, CalloutBlock, CodeBlock, EmptyBlock, HeadingBlock, TextBlock, ToDoBlock,
    TypedBlock,
};
use notion_cli::types::common::Color;
use notion_cli::types::rich_text::{
    Annotations, RichText, RichTextContent, TextContent,
};
use notion_cli::validation::BlockId;

use proptest::prelude::*;

// === Strategies ===========================================================

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
    ]
}

fn arb_rich_text() -> impl Strategy<Value = RichText> {
    "[a-zA-Z0-9 ]{0,30}".prop_map(|s| RichText {
        content: RichTextContent::Text {
            text: TextContent { content: s.clone(), link: None },
        },
        annotations: Annotations::default(),
        plain_text: s,
        href: None,
    })
}

fn arb_text_block() -> impl Strategy<Value = TextBlock> {
    (prop::collection::vec(arb_rich_text(), 0..3), arb_color())
        .prop_map(|(rich_text, color)| TextBlock { rich_text, color })
}

fn arb_heading_block() -> impl Strategy<Value = HeadingBlock> {
    (
        prop::collection::vec(arb_rich_text(), 0..3),
        arb_color(),
        any::<bool>(),
    )
        .prop_map(|(rich_text, color, is_toggleable)| HeadingBlock {
            rich_text,
            color,
            is_toggleable,
        })
}

fn arb_to_do_block() -> impl Strategy<Value = ToDoBlock> {
    (
        prop::collection::vec(arb_rich_text(), 0..3),
        arb_color(),
        any::<bool>(),
    )
        .prop_map(|(rich_text, color, checked)| ToDoBlock {
            rich_text,
            color,
            checked,
        })
}

fn arb_code_block() -> impl Strategy<Value = CodeBlock> {
    (
        prop::collection::vec(arb_rich_text(), 0..3),
        prop::collection::vec(arb_rich_text(), 0..2),
        prop_oneof![
            Just("rust".to_string()),
            Just("python".to_string()),
            Just("plain text".to_string()),
            Just("javascript".to_string()),
        ],
    )
        .prop_map(|(rich_text, caption, language)| CodeBlock {
            rich_text,
            caption,
            language,
        })
}

fn arb_callout_block() -> impl Strategy<Value = CalloutBlock> {
    (prop::collection::vec(arb_rich_text(), 0..3), arb_color())
        .prop_map(|(rich_text, color)| CalloutBlock {
            rich_text,
            color,
            icon: None,
        })
}

fn arb_block_body() -> impl Strategy<Value = BlockBody> {
    prop_oneof![
        arb_text_block().prop_map(|t| BlockBody::Paragraph { paragraph: t }),
        arb_heading_block().prop_map(|h| BlockBody::Heading1 { heading_1: h }),
        arb_heading_block().prop_map(|h| BlockBody::Heading2 { heading_2: h }),
        arb_heading_block().prop_map(|h| BlockBody::Heading3 { heading_3: h }),
        arb_text_block()
            .prop_map(|t| BlockBody::BulletedListItem { bulleted_list_item: t }),
        arb_text_block()
            .prop_map(|t| BlockBody::NumberedListItem { numbered_list_item: t }),
        arb_to_do_block().prop_map(|t| BlockBody::ToDo { to_do: t }),
        arb_text_block().prop_map(|t| BlockBody::Toggle { toggle: t }),
        arb_code_block().prop_map(|c| BlockBody::Code { code: c }),
        arb_text_block().prop_map(|t| BlockBody::Quote { quote: t }),
        arb_callout_block().prop_map(|c| BlockBody::Callout { callout: c }),
        Just(BlockBody::Divider { divider: EmptyBlock {} }),
    ]
}

fn arb_typed_block() -> impl Strategy<Value = TypedBlock> {
    ("[0-9a-f]{32}", arb_block_body()).prop_map(|(id_hex, body)| TypedBlock {
        id: BlockId::parse(&id_hex).unwrap(),
        created_time: "2026-04-17T10:00:00.000Z".into(),
        last_edited_time: "2026-04-17T10:00:00.000Z".into(),
        has_children: false,
        archived: false,
        in_trash: false,
        parent: None,
        created_by: None,
        last_edited_by: None,
        body,
    })
}

// === Roundtrip proptests ==================================================

proptest! {
    #[test]
    fn block_body_roundtrip_all_12(body in arb_block_body()) {
        let json = serde_json::to_value(&body).expect("serialise");
        let back: BlockBody = serde_json::from_value(json).expect("deserialise");
        prop_assert_eq!(body, back);
    }

    #[test]
    fn typed_block_roundtrip(block in arb_typed_block()) {
        let json = serde_json::to_value(&block).expect("serialise");
        let back: TypedBlock = serde_json::from_value(json).expect("deserialise");
        prop_assert_eq!(block, back);
    }

    #[test]
    fn block_wrapper_roundtrip_known(block in arb_typed_block()) {
        let wrapped = Block::known(block.clone());
        let json = serde_json::to_value(&wrapped).expect("serialise");
        let back: Block = serde_json::from_value(json).expect("deserialise");
        prop_assert_eq!(Block::known(block), back);
    }

    #[test]
    fn block_body_type_discriminator_present(body in arb_block_body()) {
        let json = serde_json::to_value(&body).expect("serialise");
        let obj = json.as_object().expect("object");
        prop_assert!(obj.contains_key("type"), "missing 'type' tag: {:?}", json);
    }
}

// === Graceful degradation ================================================

#[test]
fn unknown_block_type_falls_through_to_raw() {
    let json = serde_json::json!({
        "object": "block",
        "id": "abcdef0123456789abcdef0123456789",
        "created_time": "2026-04-17T10:00:00.000Z",
        "last_edited_time": "2026-04-17T10:00:00.000Z",
        "has_children": false,
        "archived": false,
        "in_trash": false,
        "type": "future_block_type_not_modelled",
        "future_block_type_not_modelled": {"some": "data"}
    });
    let back: Block = serde_json::from_value(json.clone()).unwrap();
    match back {
        Block::Raw(v) => assert_eq!(v, json),
        Block::Known(b) => panic!("expected Raw, got Known({b:?})"),
    }
}

#[test]
fn known_block_prefers_typed_variant() {
    let json = serde_json::json!({
        "object": "block",
        "id": "abcdef0123456789abcdef0123456789",
        "created_time": "2026-04-17T10:00:00.000Z",
        "last_edited_time": "2026-04-17T10:00:00.000Z",
        "has_children": false,
        "archived": false,
        "in_trash": false,
        "type": "paragraph",
        "paragraph": {"rich_text": [], "color": "default"}
    });
    let back: Block = serde_json::from_value(json).unwrap();
    assert!(back.is_writable(), "must parse as Known");
    assert!(matches!(
        back.as_known().map(|b| &b.body),
        Some(BlockBody::Paragraph { .. }),
    ));
}

#[test]
fn raw_preserves_unknown_block_payload() {
    let json = serde_json::json!({
        "object": "block",
        "id": "abcdef0123456789abcdef0123456789",
        "created_time": "2026-04-17T10:00:00.000Z",
        "last_edited_time": "2026-04-17T10:00:00.000Z",
        "has_children": true,
        "archived": false,
        "in_trash": false,
        "type": "synced_block",
        "synced_block": {
            "synced_from": {"block_id": "fedcba9876543210fedcba9876543210"},
            "deep": {"nested": ["data"]}
        }
    });
    let back: Block = serde_json::from_value(json.clone()).unwrap();
    if let Block::Raw(v) = back {
        assert_eq!(v, json, "raw payload must be preserved byte-for-byte");
    } else {
        panic!("expected Raw for synced_block");
    }
}

// === Concrete wire-format fixtures =======================================

#[test]
fn paragraph_wire_format() {
    let json = serde_json::json!({
        "object": "block",
        "id": "abcdef0123456789abcdef0123456789",
        "parent": {"type": "page_id", "page_id": "11111111111111111111111111111111"},
        "created_time": "2026-04-17T10:00:00.000Z",
        "last_edited_time": "2026-04-17T10:00:00.000Z",
        "created_by": {"object": "user", "id": "22222222222222222222222222222222"},
        "last_edited_by": {"object": "user", "id": "22222222222222222222222222222222"},
        "has_children": false,
        "archived": false,
        "in_trash": false,
        "type": "paragraph",
        "paragraph": {
            "rich_text": [{
                "type": "text",
                "text": {"content": "Hello"},
                "annotations": {
                    "bold": false, "italic": false, "strikethrough": false,
                    "underline": false, "code": false, "color": "default"
                },
                "plain_text": "Hello"
            }],
            "color": "default"
        }
    });
    let block: Block = serde_json::from_value(json).unwrap();
    let typed = block.as_known().expect("known");
    assert!(matches!(
        &typed.body,
        BlockBody::Paragraph { paragraph } if paragraph.rich_text.len() == 1
    ));
}

#[test]
fn divider_wire_format_empty_object() {
    let json = serde_json::json!({
        "object": "block",
        "id": "abcdef0123456789abcdef0123456789",
        "created_time": "2026-04-17T10:00:00.000Z",
        "last_edited_time": "2026-04-17T10:00:00.000Z",
        "has_children": false,
        "archived": false,
        "in_trash": false,
        "type": "divider",
        "divider": {}
    });
    let block: Block = serde_json::from_value(json).unwrap();
    assert!(matches!(
        block.as_known().map(|b| &b.body),
        Some(BlockBody::Divider { .. }),
    ));
}

#[test]
fn code_block_with_language() {
    let json = serde_json::json!({
        "object": "block",
        "id": "abcdef0123456789abcdef0123456789",
        "created_time": "2026-04-17T10:00:00.000Z",
        "last_edited_time": "2026-04-17T10:00:00.000Z",
        "has_children": false,
        "archived": false,
        "in_trash": false,
        "type": "code",
        "code": {
            "rich_text": [{
                "type": "text",
                "text": {"content": "let x = 1;"},
                "annotations": {"bold": false, "italic": false, "strikethrough": false, "underline": false, "code": false, "color": "default"},
                "plain_text": "let x = 1;"
            }],
            "caption": [],
            "language": "rust"
        }
    });
    let block: Block = serde_json::from_value(json).unwrap();
    if let Some(BlockBody::Code { code }) = block.as_known().map(|b| &b.body) {
        assert_eq!(code.language, "rust");
    } else {
        panic!("expected Code variant");
    }
}

#[test]
fn to_do_checked_field() {
    let json = serde_json::json!({
        "object": "block",
        "id": "abcdef0123456789abcdef0123456789",
        "created_time": "2026-04-17T10:00:00.000Z",
        "last_edited_time": "2026-04-17T10:00:00.000Z",
        "has_children": false,
        "archived": false,
        "in_trash": false,
        "type": "to_do",
        "to_do": {
            "rich_text": [],
            "color": "default",
            "checked": true
        }
    });
    let block: Block = serde_json::from_value(json).unwrap();
    if let Some(BlockBody::ToDo { to_do }) = block.as_known().map(|b| &b.body) {
        assert!(to_do.checked);
    } else {
        panic!("expected ToDo variant");
    }
}

// === Convenience constructors ============================================

#[test]
fn body_paragraph_constructor() {
    let b = BlockBody::paragraph("hello");
    let json = serde_json::to_value(&b).unwrap();
    assert_eq!(json["type"], "paragraph");
    assert_eq!(
        json["paragraph"]["rich_text"][0]["text"]["content"],
        "hello",
    );
}

#[test]
fn body_heading_constructors() {
    for (h, expected_tag) in [
        (BlockBody::heading_1("H1"), "heading_1"),
        (BlockBody::heading_2("H2"), "heading_2"),
        (BlockBody::heading_3("H3"), "heading_3"),
    ] {
        let json = serde_json::to_value(&h).unwrap();
        assert_eq!(json["type"], expected_tag);
    }
}

#[test]
fn body_to_do_with_checked() {
    let b = BlockBody::to_do("task", true);
    let json = serde_json::to_value(&b).unwrap();
    assert_eq!(json["type"], "to_do");
    assert_eq!(json["to_do"]["checked"], true);
}

#[test]
fn body_divider_is_empty_object() {
    let b = BlockBody::divider();
    let json = serde_json::to_value(&b).unwrap();
    assert_eq!(json["type"], "divider");
    assert_eq!(json["divider"], serde_json::json!({}));
}

// === Block::as_known / is_writable contract ==============================

#[test]
fn block_raw_is_not_writable() {
    let b = Block::Raw(serde_json::json!({"type": "unknown"}));
    assert!(!b.is_writable());
    assert!(b.as_known().is_none());
}

#[test]
fn block_known_is_writable() {
    let typed = TypedBlock {
        id: BlockId::parse("abcdef0123456789abcdef0123456789").unwrap(),
        created_time: "2026-04-17T00:00:00.000Z".into(),
        last_edited_time: "2026-04-17T00:00:00.000Z".into(),
        has_children: false,
        archived: false,
        in_trash: false,
        parent: None,
        created_by: None,
        last_edited_by: None,
        body: BlockBody::paragraph("hi"),
    };
    let b = Block::known(typed);
    assert!(b.is_writable());
    assert!(b.as_known().is_some());
}
