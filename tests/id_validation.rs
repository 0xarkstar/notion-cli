//! Adversarial ID validation suite.
//!
//! Targets two attack surfaces:
//! 1. Format-parse rejecting valid Notion inputs (URL, dashed UUID).
//! 2. Format-parse accepting structurally invalid inputs.

use notion_cli::validation::DatabaseId;

// === Strict parse() ========================================================

#[test]
fn parse_accepts_32_hex_no_dash() {
    let id = DatabaseId::parse("abcdef0123456789abcdef0123456789").unwrap();
    assert_eq!(id.as_str(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn parse_accepts_dashed_uuid() {
    let id = DatabaseId::parse("abcdef01-2345-6789-abcd-ef0123456789").unwrap();
    assert_eq!(id.as_str(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn parse_normalises_to_lowercase() {
    let id = DatabaseId::parse("ABCDEF0123456789ABCDEF0123456789").unwrap();
    assert_eq!(id.as_str(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn parse_tolerates_dashes_in_wrong_positions() {
    // Matches Notion's own tolerance — strip-and-validate.
    let id = DatabaseId::parse("abc-def0123456789abcdef012345678-9").unwrap();
    assert_eq!(id.as_str(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn parse_rejects_31_chars() {
    assert!(DatabaseId::parse("abcdef0123456789abcdef012345678").is_err());
}

#[test]
fn parse_rejects_33_chars() {
    assert!(DatabaseId::parse("abcdef0123456789abcdef01234567890").is_err());
}

#[test]
fn parse_rejects_non_hex() {
    assert!(DatabaseId::parse("ghijklmnopqrstuvwxyzabcdef012345").is_err());
}

#[test]
fn parse_rejects_empty() {
    assert!(DatabaseId::parse("").is_err());
}

#[test]
fn parse_rejects_urls() {
    // parse() is strict; URLs must go through from_url_or_id.
    assert!(
        DatabaseId::parse("https://notion.so/Page-abcdef0123456789abcdef0123456789")
            .is_err()
    );
}

#[test]
fn parse_rejects_path_traversal_literals() {
    // IDs are not filesystem paths; path-traversal input simply fails
    // the format check on length and hex grounds.
    assert!(DatabaseId::parse("../../../etc/passwd").is_err());
    assert!(DatabaseId::parse("..").is_err());
    assert!(DatabaseId::parse(".".repeat(32).as_str()).is_err());
}

#[test]
fn parse_rejects_control_characters() {
    assert!(DatabaseId::parse("abcdef\x00123456789abcdef012345678").is_err());
    assert!(
        DatabaseId::parse("abcdef0123456789abcdef012345678\x1b").is_err(),
        "ESC char should fail hex check",
    );
}

#[test]
fn parse_rejects_null_bytes() {
    let nulls = "\0".repeat(32);
    assert!(DatabaseId::parse(&nulls).is_err());
}

#[test]
fn parse_rejects_unicode_homoglyphs() {
    // Cyrillic 'а' (U+0430) looks like Latin 'a' — must be rejected.
    assert!(DatabaseId::parse("аbcdef0123456789abcdef0123456789").is_err());
}

// === URL-accepting from_url_or_id() =======================================

#[test]
fn from_url_or_id_accepts_plain_id() {
    let id = DatabaseId::from_url_or_id("abcdef0123456789abcdef0123456789").unwrap();
    assert_eq!(id.as_str(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn from_url_or_id_accepts_dashed() {
    let id = DatabaseId::from_url_or_id("abcdef01-2345-6789-abcd-ef0123456789").unwrap();
    assert_eq!(id.as_str(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn from_url_or_id_accepts_notion_url_with_title() {
    let id = DatabaseId::from_url_or_id(
        "https://www.notion.so/workspace/My-Page-Title-abcdef0123456789abcdef0123456789",
    )
    .unwrap();
    assert_eq!(id.as_str(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn from_url_or_id_accepts_url_with_query() {
    let id = DatabaseId::from_url_or_id(
        "https://notion.so/Page-abcdef0123456789abcdef0123456789?v=xyz&k=1",
    )
    .unwrap();
    assert_eq!(id.as_str(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn from_url_or_id_accepts_url_with_fragment() {
    let id = DatabaseId::from_url_or_id(
        "https://notion.so/Page-abcdef0123456789abcdef0123456789#heading",
    )
    .unwrap();
    assert_eq!(id.as_str(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn from_url_or_id_accepts_url_with_dashed_id() {
    let id =
        DatabaseId::from_url_or_id("https://notion.so/abcdef01-2345-6789-abcd-ef0123456789")
            .unwrap();
    assert_eq!(id.as_str(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn from_url_or_id_rejects_url_without_hex_tail() {
    assert!(DatabaseId::from_url_or_id("https://notion.so/NoIdHere").is_err());
}

#[test]
fn from_url_or_id_rejects_url_with_too_short_hex_tail() {
    assert!(
        DatabaseId::from_url_or_id("https://notion.so/Page-abcdef01").is_err(),
        "8 hex chars should not pass",
    );
}

// === Normalisation round-trip =============================================

#[test]
fn as_dashed_produces_canonical_uuid_form() {
    let id = DatabaseId::parse("abcdef0123456789abcdef0123456789").unwrap();
    assert_eq!(id.as_dashed(), "abcdef01-2345-6789-abcd-ef0123456789");
}

#[test]
fn display_uses_normalised_no_dash_form() {
    let id = DatabaseId::parse("ABCDEF01-2345-6789-ABCD-EF0123456789").unwrap();
    assert_eq!(format!("{id}"), "abcdef0123456789abcdef0123456789");
}

#[test]
fn parse_then_as_dashed_then_parse_is_idempotent() {
    let raw = "abcdef0123456789abcdef0123456789";
    let id = DatabaseId::parse(raw).unwrap();
    let dashed = id.as_dashed();
    let reparsed = DatabaseId::parse(&dashed).unwrap();
    assert_eq!(id, reparsed);
}

// === Serde round-trip =====================================================

#[test]
fn serde_serialises_to_plain_string() {
    let id = DatabaseId::parse("abcdef0123456789abcdef0123456789").unwrap();
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"abcdef0123456789abcdef0123456789\"");
}

#[test]
fn serde_deserialises_plain_id() {
    let id: DatabaseId =
        serde_json::from_str("\"abcdef0123456789abcdef0123456789\"").unwrap();
    assert_eq!(id.as_str(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn serde_deserialises_dashed_id() {
    let id: DatabaseId =
        serde_json::from_str("\"abcdef01-2345-6789-abcd-ef0123456789\"").unwrap();
    assert_eq!(id.as_str(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn serde_deserialises_url_form() {
    let id: DatabaseId = serde_json::from_str(
        "\"https://notion.so/Page-abcdef0123456789abcdef0123456789\"",
    )
    .unwrap();
    assert_eq!(id.as_str(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn serde_rejects_malformed_id() {
    let err = serde_json::from_str::<DatabaseId>("\"not-an-id\"");
    assert!(err.is_err());
}

// === FromStr ==============================================================

#[test]
fn from_str_accepts_url() {
    let id: DatabaseId =
        "https://notion.so/abcdef0123456789abcdef0123456789".parse().unwrap();
    assert_eq!(id.as_str(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn from_str_accepts_plain() {
    let id: DatabaseId = "abcdef0123456789abcdef0123456789".parse().unwrap();
    assert_eq!(id.as_str(), "abcdef0123456789abcdef0123456789");
}
