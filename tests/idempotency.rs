//! Idempotency-Key header tests.

use std::num::NonZeroU32;
use std::time::Duration;

use notion_cli::api::{ClientConfig, NotionClient};
use notion_cli::api::data_source::{CreateDataSourceParent, CreateDataSourceRequest};
use notion_cli::api::page::{CreatePageRequest, PageParent, UpdatePageRequest};
use notion_cli::config::NotionToken;
use notion_cli::validation::{DatabaseId, DataSourceId, PageId};
use serde_json::json;
use wiremock::matchers::{header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_TOKEN: &str = "ntn_test_idempotency_abcdef01234567";
const PAGE_ID_HEX: &str = "11111111111111111111111111111111";
const DS_ID_HEX: &str = "fedcba9876543210fedcba9876543210";
const DB_ID_HEX: &str = "abcdef0123456789abcdef0123456789";

fn test_page_json(id: &str) -> serde_json::Value {
    json!({
        "object": "page",
        "id": id,
        "created_time": "2026-04-15T10:00:00.000Z",
        "last_edited_time": "2026-04-15T10:00:00.000Z",
        "archived": false,
        "in_trash": false,
        "url": format!("https://www.notion.so/Page-{id}"),
        "parent": {"type": "workspace", "workspace": true},
        "properties": {}
    })
}

fn test_ds_json(id: &str) -> serde_json::Value {
    json!({
        "object": "data_source",
        "id": id,
        "created_time": "2026-04-15T10:00:00.000Z",
        "last_edited_time": "2026-04-15T10:00:00.000Z",
        "name": "Test DS",
        "properties": {
            "Name": {"id": "title", "type": "title", "title": {}}
        },
        "parent": {"type": "database_id", "database_id": DB_ID_HEX}
    })
}

fn make_client(server: &MockServer) -> NotionClient {
    let config = ClientConfig {
        base_url: server.uri(),
        connect_timeout: Duration::from_secs(5),
        total_timeout: Duration::from_secs(10),
        max_response_bytes: notion_cli::api::MAX_RESPONSE_BYTES,
        rate_limit_per_sec: NonZeroU32::new(100).unwrap(),
        cache_ttl: None,
    };
    NotionClient::with_config(&NotionToken::new(TEST_TOKEN), config).unwrap()
}

#[tokio::test]
async fn post_sends_idempotency_key_header() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/pages"))
        .and(header_exists("Idempotency-Key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_page_json(PAGE_ID_HEX)))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server);
    let _ = client
        .create_page(&CreatePageRequest {
            parent: PageParent::DataSource {
                data_source_id: DataSourceId::parse(DS_ID_HEX).unwrap(),
            },
            properties: std::collections::HashMap::new(),
            children: vec![],
            icon: None,
            cover: None,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn patch_sends_idempotency_key_header() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/pages/{PAGE_ID_HEX}")))
        .and(header_exists("Idempotency-Key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_page_json(PAGE_ID_HEX)))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server);
    let _ = client
        .update_page(
            &PageId::parse(PAGE_ID_HEX).unwrap(),
            &UpdatePageRequest {
                properties: std::collections::HashMap::new(),
                archived: None,
                in_trash: None,
                icon: None,
                cover: None,
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn post_idempotency_key_is_uuid_format() {
    // Assert the auto-generated key looks like a UUID v4.
    let server = MockServer::start().await;

    // Use a responder that captures the header value.
    Mock::given(method("POST"))
        .and(path("/v1/data_sources"))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_ds_json(DS_ID_HEX)))
        .mount(&server)
        .await;

    let client = make_client(&server);
    let _ = client
        .create_data_source(&CreateDataSourceRequest {
            parent: CreateDataSourceParent::database(DatabaseId::parse(DB_ID_HEX).unwrap()),
            title: vec![],
            properties: json!({"Name": {"title": {}}}),
        })
        .await
        .unwrap();

    // Verify the request arrived with an Idempotency-Key header.
    let received = server.received_requests().await.unwrap();
    assert_eq!(received.len(), 1);
    let key = received[0]
        .headers
        .get("idempotency-key")
        .expect("Idempotency-Key header must be present");
    // Should be a 36-char UUID string (8-4-4-4-12).
    let key_str = key.to_str().expect("header must be valid UTF-8");
    assert_eq!(key_str.len(), 36, "UUID must be 36 chars: {key_str}");
    assert_eq!(
        key_str.chars().filter(|&c| c == '-').count(),
        4,
        "UUID must have 4 dashes: {key_str}"
    );
}
