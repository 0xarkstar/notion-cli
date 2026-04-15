//! Realistic-shape fixture tests against anonymised Notion API responses.
//!
//! These lock the assumption that our types can ingest the output of
//! Notion API 2026-03-11 without loss for read-path operations.

use notion_cli::types::page::Page;
use notion_cli::types::{Property, PropertyValue};

#[test]
#[allow(clippy::too_many_lines)]
fn ingests_realistic_page_response() {
    let json = serde_json::json!({
        "object": "page",
        "id": "abcdef0123456789abcdef0123456789",
        "created_time": "2026-04-15T10:00:00.000Z",
        "last_edited_time": "2026-04-15T10:30:00.000Z",
        "archived": false,
        "in_trash": false,
        "url": "https://www.notion.so/My-Page-abcdef0123456789abcdef0123456789",
        "icon": {"type": "emoji", "emoji": "📝"},
        "cover": null,
        "parent": {
            "type": "database_id",
            "database_id": "fedcba9876543210fedcba9876543210"
        },
        "properties": {
            "Name": {
                "id": "title",
                "type": "title",
                "title": [{
                    "type": "text",
                    "text": {"content": "Hello", "link": null},
                    "annotations": {
                        "bold": false, "italic": false, "strikethrough": false,
                        "underline": false, "code": false, "color": "default"
                    },
                    "plain_text": "Hello",
                    "href": null
                }]
            },
            "Status": {
                "id": "s",
                "type": "status",
                "status": {"id": "opt-1", "name": "In Progress", "color": "blue"}
            },
            "Priority": {
                "id": "p",
                "type": "select",
                "select": {"id": "opt-2", "name": "High", "color": "red"}
            },
            "Done": {
                "id": "d",
                "type": "checkbox",
                "checkbox": false
            },
            "Tags": {
                "id": "t",
                "type": "multi_select",
                "multi_select": [
                    {"id": "tag-1", "name": "rust", "color": "orange"},
                    {"id": "tag-2", "name": "notion", "color": "gray"}
                ]
            },
            "Due": {
                "id": "dd",
                "type": "date",
                "date": {"start": "2026-04-20", "end": null, "time_zone": null}
            },
            "Assignees": {
                "id": "a",
                "type": "people",
                "people": [
                    {"object": "user", "id": "11111111111111111111111111111111"}
                ]
            },
            "Notes": {
                "id": "n",
                "type": "rich_text",
                "rich_text": []
            },
            "Estimate": {
                "id": "e",
                "type": "number",
                "number": 3.5
            },
            "Contact": {
                "id": "c",
                "type": "email",
                "email": "someone@example.com"
            },
            "Phone": {
                "id": "ph",
                "type": "phone_number",
                "phone_number": "+1-555-0100"
            },
            "Link": {
                "id": "l",
                "type": "url",
                "url": "https://example.com"
            },
            "Related": {
                "id": "r",
                "type": "relation",
                "relation": [
                    {"id": "22222222222222222222222222222222"}
                ]
            },
            // Read-only variants
            "Score": {
                "id": "sc",
                "type": "formula",
                "formula": {"type": "number", "number": 42}
            },
            "Sum": {
                "id": "sm",
                "type": "rollup",
                "rollup": {"type": "number", "number": 100, "function": "sum"}
            },
            "Created": {
                "id": "ct",
                "type": "created_time",
                "created_time": "2026-04-15T10:00:00.000Z"
            },
            "Creator": {
                "id": "cb",
                "type": "created_by",
                "created_by": {"object": "user", "id": "33333333333333333333333333333333"}
            },
            "LastEdited": {
                "id": "let",
                "type": "last_edited_time",
                "last_edited_time": "2026-04-15T10:30:00.000Z"
            },
            "LastEditor": {
                "id": "leb",
                "type": "last_edited_by",
                "last_edited_by": {"object": "user", "id": "44444444444444444444444444444444"}
            },
            "Uid": {
                "id": "u",
                "type": "unique_id",
                "unique_id": {"number": 123, "prefix": "TASK"}
            },
            "Verify": {
                "id": "v",
                "type": "verification",
                "verification": {"state": "verified"}
            },
            "Files": {
                "id": "f",
                "type": "files",
                "files": []
            },
            // Future / unknown type — must fall through to Raw
            "FutureField": {
                "id": "fx",
                "type": "hypothetical_future_type",
                "hypothetical_future_type": {"some": "data"}
            }
        }
    });

    let page: Page = serde_json::from_value(json).expect("deserialise realistic page");

    assert_eq!(page.id.as_str(), "abcdef0123456789abcdef0123456789");
    assert!(!page.archived);
    assert!(!page.in_trash);

    // 22 known properties + 1 FutureField (Raw)
    assert_eq!(page.properties.len(), 23);

    // Spot-check each write-path variant
    let title = page.properties.get("Name").unwrap();
    assert!(matches!(title, Property::Known(PropertyValue::Title { .. })));

    let checkbox = page.properties.get("Done").unwrap();
    assert!(matches!(
        checkbox,
        Property::Known(PropertyValue::Checkbox { checkbox: false }),
    ));

    // Read-only variants should also be Known (modeled, even if opaque inner)
    assert!(matches!(
        page.properties.get("Score").unwrap(),
        Property::Known(PropertyValue::Formula { .. }),
    ));
    assert!(matches!(
        page.properties.get("Uid").unwrap(),
        Property::Known(PropertyValue::UniqueId { .. }),
    ));

    // Unknown type falls through to Raw
    assert!(
        matches!(page.properties.get("FutureField").unwrap(), Property::Raw(_)),
        "hypothetical_future_type must land in Raw",
    );

    // Writable filter: 22 known, 1 raw filtered out
    let writable_count = page
        .properties
        .values()
        .filter(|p| p.is_writable())
        .count();
    assert_eq!(writable_count, 22);
}

#[test]
fn round_trips_known_properties_through_page() {
    let json = serde_json::json!({
        "object": "page",
        "id": "abcdef0123456789abcdef0123456789",
        "created_time": "2026-04-15T10:00:00.000Z",
        "last_edited_time": "2026-04-15T10:30:00.000Z",
        "archived": false,
        "in_trash": false,
        "url": "https://www.notion.so/Page",
        "parent": {"type": "page_id", "page_id": "fedcba9876543210fedcba9876543210"},
        "properties": {
            "Done": {"id": "d", "type": "checkbox", "checkbox": true}
        }
    });

    let page: Page = serde_json::from_value(json).unwrap();
    let back = serde_json::to_value(&page).unwrap();
    let round: Page = serde_json::from_value(back).unwrap();
    assert_eq!(page.properties.len(), round.properties.len());
    assert!(matches!(
        round.properties.get("Done").unwrap(),
        Property::Known(PropertyValue::Checkbox { checkbox: true }),
    ));
}
