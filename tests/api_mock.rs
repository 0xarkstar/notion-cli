//! Wiremock-driven API tests — the HTTP stack contract.
//!
//! These tests spin up an HTTP mock server, point [`NotionClient`] at
//! its base URL, and assert:
//! - Headers: `Authorization: Bearer …`, `Notion-Version`, content-type.
//! - 200 deserialises into the expected typed response.
//! - 401/404/400/5xx map to the right `ApiError` variants.
//! - 429 with `Retry-After` triggers a retry and succeeds.
//! - Oversized response (>10 MiB) fails with `BodyTooLarge`, not OOM.
//! - The Authorization token is never echoed in error `Debug`/`Display`.
//! - `create_data_source` (the-bug endpoint) hits the right path.

use std::num::NonZeroU32;
use std::time::Duration;

use std::collections::HashMap;

use notion_cli::api::{ApiError, ClientConfig, NotionClient, NOTION_API_VERSION};
use notion_cli::api::data_source::{
    CreateDataSourceParent, CreateDataSourceRequest, QueryDataSourceRequest,
};
use notion_cli::api::database::{
    CreateDatabaseParent, CreateDatabaseRequest, InitialDataSource,
};
use notion_cli::api::page::{CreatePageRequest, PageParent, UpdatePageRequest};
use notion_cli::config::NotionToken;
use notion_cli::types::icon::Icon;
use notion_cli::types::property::PropertyValue;
use notion_cli::types::property_schema::{EmptyConfig, PropertySchema, SelectConfig};
use notion_cli::types::common::SelectOption;
use notion_cli::types::rich_text::RichText;
use notion_cli::validation::{DataSourceId, DatabaseId, PageId};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_TOKEN: &str = "ntn_test_super_secret_abcdef0123456789";
const DB_ID_HEX: &str = "abcdef0123456789abcdef0123456789";
const DS_ID_HEX: &str = "fedcba9876543210fedcba9876543210";
const PAGE_ID_HEX: &str = "11111111111111111111111111111111";

fn test_page_json(id: &str) -> serde_json::Value {
    json!({
        "object": "page",
        "id": id,
        "created_time": "2026-04-15T10:00:00.000Z",
        "last_edited_time": "2026-04-15T10:00:00.000Z",
        "archived": false,
        "in_trash": false,
        "url": format!("https://www.notion.so/Page-{id}"),
        "parent": {"type": "data_source_id", "data_source_id": DS_ID_HEX},
        "properties": {
            "Done": {"id": "d", "type": "checkbox", "checkbox": true}
        }
    })
}

fn test_data_source_json(id: &str) -> serde_json::Value {
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
        rate_limit_per_sec: NonZeroU32::new(100).unwrap(), // fast for tests
    };
    NotionClient::with_config(&NotionToken::new(TEST_TOKEN), config).unwrap()
}

// === Headers ==============================================================

#[tokio::test]
async fn sends_authorization_and_notion_version_headers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/pages/{PAGE_ID_HEX}")))
        .and(header("Authorization", format!("Bearer {TEST_TOKEN}").as_str()))
        .and(header("Notion-Version", NOTION_API_VERSION))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_page_json(PAGE_ID_HEX)))
        .mount(&server)
        .await;

    let client = make_client(&server);
    let page_id = PageId::parse(PAGE_ID_HEX).unwrap();
    let page = client.retrieve_page(&page_id).await.unwrap();
    assert_eq!(page.id.as_str(), PAGE_ID_HEX);
}

// === Status code classification ===========================================

#[tokio::test]
async fn maps_401_to_unauthorized() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(401).set_body_json(json!({
                "object": "error",
                "status": 401,
                "code": "unauthorized",
                "message": "API token is invalid."
            })),
        )
        .mount(&server)
        .await;

    let client = make_client(&server);
    let err = client
        .retrieve_page(&PageId::parse(PAGE_ID_HEX).unwrap())
        .await
        .unwrap_err();
    assert!(matches!(err, ApiError::Unauthorized), "got {err:?}");
}

#[tokio::test]
async fn maps_404_to_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(json!({
                "object": "error",
                "code": "object_not_found",
                "message": "Page not found."
            })),
        )
        .mount(&server)
        .await;

    let client = make_client(&server);
    let err = client
        .retrieve_page(&PageId::parse(PAGE_ID_HEX).unwrap())
        .await
        .unwrap_err();
    assert!(matches!(err, ApiError::NotFound), "got {err:?}");
}

#[tokio::test]
async fn maps_400_to_validation_with_code_and_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(400).set_body_json(json!({
                "object": "error",
                "code": "validation_error",
                "message": "properties.Name.title is required."
            })),
        )
        .mount(&server)
        .await;

    let client = make_client(&server);
    let err = client
        .create_data_source(&CreateDataSourceRequest {
            parent: CreateDataSourceParent::database(DatabaseId::parse(DB_ID_HEX).unwrap()),
            title: vec![],
            properties: json!({}),
        })
        .await
        .unwrap_err();

    match err {
        ApiError::Validation { code, message } => {
            assert_eq!(code, "validation_error");
            assert!(message.contains("title is required"), "got: {message}");
        }
        other => panic!("expected Validation, got {other:?}"),
    }
}

#[tokio::test]
async fn maps_500_to_server_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let client = make_client(&server);
    let err = client
        .retrieve_page(&PageId::parse(PAGE_ID_HEX).unwrap())
        .await
        .unwrap_err();
    assert!(matches!(err, ApiError::ServerError { status: 503, .. }), "got {err:?}");
}

// === 429 retry behaviour ==================================================

#[tokio::test]
async fn retries_on_429_with_retry_after_then_succeeds() {
    let server = MockServer::start().await;
    // First: 429 with Retry-After: 1
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(429).insert_header("Retry-After", "1"),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    // Then: 200 OK
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_page_json(PAGE_ID_HEX)))
        .mount(&server)
        .await;

    let client = make_client(&server);
    let start = std::time::Instant::now();
    let page = client
        .retrieve_page(&PageId::parse(PAGE_ID_HEX).unwrap())
        .await
        .unwrap();
    let elapsed = start.elapsed();
    assert_eq!(page.id.as_str(), PAGE_ID_HEX);
    assert!(
        elapsed >= Duration::from_millis(900),
        "retry must honour Retry-After; elapsed={elapsed:?}",
    );
}

#[tokio::test]
async fn exhausts_retries_and_returns_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
        .mount(&server)
        .await;

    let client = make_client(&server);
    let err = client
        .retrieve_page(&PageId::parse(PAGE_ID_HEX).unwrap())
        .await
        .unwrap_err();
    assert!(
        matches!(err, ApiError::RateLimited { .. }),
        "expected RateLimited after retry exhaustion, got {err:?}",
    );
}

// === Response size cap ====================================================

#[tokio::test]
async fn oversized_response_fails_with_body_too_large() {
    let server = MockServer::start().await;
    // 2048 bytes real body; client capped at 1024 → must reject.
    let huge = "x".repeat(2048);
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(huge))
        .mount(&server)
        .await;

    let config = ClientConfig {
        base_url: server.uri(),
        connect_timeout: Duration::from_secs(5),
        total_timeout: Duration::from_secs(10),
        max_response_bytes: 1024,
        rate_limit_per_sec: NonZeroU32::new(100).unwrap(),
    };
    let client = NotionClient::with_config(&NotionToken::new(TEST_TOKEN), config).unwrap();
    let err = client
        .retrieve_page(&PageId::parse(PAGE_ID_HEX).unwrap())
        .await
        .unwrap_err();
    assert!(
        matches!(err, ApiError::BodyTooLarge { limit_bytes: 1024 }),
        "got {err:?}",
    );
}

// === Token scrubbing ======================================================

#[tokio::test]
async fn token_is_never_exposed_in_error_debug() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let client = make_client(&server);
    let err = client
        .retrieve_page(&PageId::parse(PAGE_ID_HEX).unwrap())
        .await
        .unwrap_err();
    let dbg = format!("{err:?}");
    let disp = format!("{err}");
    assert!(!dbg.contains(TEST_TOKEN), "token leaked in Debug: {dbg}");
    assert!(!disp.contains(TEST_TOKEN), "token leaked in Display: {disp}");
}

#[tokio::test]
async fn notion_token_debug_shows_only_prefix() {
    let token = NotionToken::new("ntn_super_secret_rest_of_token_0123");
    let dbg = format!("{token:?}");
    assert!(dbg.starts_with("NotionToken(ntn_"));
    assert!(!dbg.contains("super_secret"));
}

// === The-bug endpoint: create_data_source =================================

#[tokio::test]
async fn create_data_source_hits_correct_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/data_sources"))
        .and(header("Notion-Version", NOTION_API_VERSION))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_data_source_json(DS_ID_HEX)))
        .mount(&server)
        .await;

    let client = make_client(&server);
    let ds = client
        .create_data_source(&CreateDataSourceRequest {
            parent: CreateDataSourceParent::database(DatabaseId::parse(DB_ID_HEX).unwrap()),
            title: vec![],
            properties: json!({"Name": {"title": {}}}),
        })
        .await
        .unwrap();
    assert_eq!(ds.id.as_str(), DS_ID_HEX);
}

// === Admin: create_database (v0.3) ========================================

fn test_database_json(id: &str) -> serde_json::Value {
    json!({
        "object": "database",
        "id": id,
        "created_time": "2026-04-22T10:00:00.000Z",
        "last_edited_time": "2026-04-22T10:00:00.000Z",
        "title": [],
        "description": [],
        "archived": false,
        "in_trash": false,
        "properties": {
            "Name": {"type": "title", "title": {}}
        },
        "data_sources": [
            {"id": DS_ID_HEX, "name": "Default"}
        ]
    })
}

#[tokio::test]
async fn create_database_hits_correct_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/databases"))
        .and(header("Notion-Version", NOTION_API_VERSION))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(test_database_json(DB_ID_HEX)),
        )
        .mount(&server)
        .await;

    let client = make_client(&server);
    let mut props = HashMap::new();
    props.insert(
        "Name".to_string(),
        PropertySchema::Title { title: EmptyConfig {} },
    );
    let req = CreateDatabaseRequest {
        parent: CreateDatabaseParent::page(PageId::parse(PAGE_ID_HEX).unwrap()),
        title: RichText::plain("Test DB"),
        initial_data_source: InitialDataSource { properties: props },
        icon: None,
        cover: None,
        is_inline: None,
    };
    let db = client.create_database(&req).await.unwrap();
    assert_eq!(db.id.as_str(), DB_ID_HEX);
}

#[tokio::test]
async fn create_database_serialises_initial_data_source_and_title() {
    let server = MockServer::start().await;
    // Match the exact wire body: annotations serialise with default
    // bool/color fields on every run; link + href skip when None.
    let expected_body = json!({
        "parent": {"type": "page_id", "page_id": PAGE_ID_HEX},
        "title": [{
            "type": "text",
            "text": {"content": "Inventory"},
            "annotations": {
                "bold": false, "italic": false, "strikethrough": false,
                "underline": false, "code": false, "color": "default"
            },
            "plain_text": "Inventory"
        }],
        "initial_data_source": {
            "properties": {
                "Name": {"type": "title", "title": {}},
                "Priority": {
                    "type": "select",
                    "select": {"options": [{"name": "High"}, {"name": "Low"}]}
                }
            }
        },
        "icon": {"type": "emoji", "emoji": "📦"}
    });
    Mock::given(method("POST"))
        .and(path("/v1/databases"))
        .and(body_json(expected_body))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(test_database_json(DB_ID_HEX)),
        )
        .mount(&server)
        .await;

    let client = make_client(&server);
    let mut props = HashMap::new();
    props.insert(
        "Name".to_string(),
        PropertySchema::Title { title: EmptyConfig {} },
    );
    props.insert(
        "Priority".to_string(),
        PropertySchema::Select {
            select: SelectConfig {
                options: vec![
                    SelectOption { id: None, name: "High".into(), color: None },
                    SelectOption { id: None, name: "Low".into(), color: None },
                ],
            },
        },
    );
    let req = CreateDatabaseRequest {
        parent: CreateDatabaseParent::page(PageId::parse(PAGE_ID_HEX).unwrap()),
        title: RichText::plain("Inventory"),
        initial_data_source: InitialDataSource { properties: props },
        icon: Some(Icon::emoji("📦")),
        cover: None,
        is_inline: None,
    };
    let db = client.create_database(&req).await.unwrap();
    assert_eq!(db.id.as_str(), DB_ID_HEX);
}

#[test]
fn create_database_validate_local_rejects_missing_title_property() {
    let mut props = HashMap::new();
    props.insert(
        "Whatever".to_string(),
        PropertySchema::Checkbox { checkbox: EmptyConfig {} },
    );
    let req = CreateDatabaseRequest {
        parent: CreateDatabaseParent::page(PageId::parse(PAGE_ID_HEX).unwrap()),
        title: RichText::plain("Test"),
        initial_data_source: InitialDataSource { properties: props },
        icon: None,
        cover: None,
        is_inline: None,
    };
    let err = req.validate_local().unwrap_err();
    assert!(err.to_lowercase().contains("title"), "expected title-prop hint: {err}");
}

#[test]
fn create_database_validate_local_rejects_empty_properties() {
    let req = CreateDatabaseRequest {
        parent: CreateDatabaseParent::page(PageId::parse(PAGE_ID_HEX).unwrap()),
        title: RichText::plain("Test"),
        initial_data_source: InitialDataSource { properties: HashMap::new() },
        icon: None,
        cover: None,
        is_inline: None,
    };
    let err = req.validate_local().unwrap_err();
    assert!(err.to_lowercase().contains("empty"), "expected empty hint: {err}");
}

#[test]
fn create_database_validate_local_accepts_title_present() {
    let mut props = HashMap::new();
    props.insert(
        "Name".to_string(),
        PropertySchema::Title { title: EmptyConfig {} },
    );
    let req = CreateDatabaseRequest {
        parent: CreateDatabaseParent::page(PageId::parse(PAGE_ID_HEX).unwrap()),
        title: RichText::plain("Test"),
        initial_data_source: InitialDataSource { properties: props },
        icon: None,
        cover: None,
        is_inline: None,
    };
    assert!(req.validate_local().is_ok());
}

// === end create_database ==================================================

#[tokio::test]
async fn create_data_source_sends_typed_parent() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/data_sources"))
        .and(body_json(json!({
            "parent": {"type": "database_id", "database_id": DB_ID_HEX},
            "properties": {"Name": {"title": {}}}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_data_source_json(DS_ID_HEX)))
        .mount(&server)
        .await;

    let client = make_client(&server);
    let ds = client
        .create_data_source(&CreateDataSourceRequest {
            parent: CreateDataSourceParent::database(DatabaseId::parse(DB_ID_HEX).unwrap()),
            title: vec![],
            properties: json!({"Name": {"title": {}}}),
        })
        .await
        .unwrap();
    assert_eq!(ds.id.as_str(), DS_ID_HEX);
}

// === Query data source (the read-path endpoint) ==========================

#[tokio::test]
async fn query_data_source_returns_paginated_pages() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/data_sources/{DS_ID_HEX}/query")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "results": [test_page_json(PAGE_ID_HEX)],
            "has_more": true,
            "next_cursor": "cursor_xyz"
        })))
        .mount(&server)
        .await;

    let client = make_client(&server);
    let ds_id = DataSourceId::parse(DS_ID_HEX).unwrap();
    let resp = client
        .query_data_source(&ds_id, &QueryDataSourceRequest::default())
        .await
        .unwrap();
    assert_eq!(resp.results.len(), 1);
    assert!(resp.has_more);
    assert_eq!(resp.next_cursor.as_deref(), Some("cursor_xyz"));
}

#[tokio::test]
async fn query_data_source_with_filter_and_cursor() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/data_sources/{DS_ID_HEX}/query")))
        .and(body_json(json!({
            "filter": {"property": "Done", "checkbox": {"equals": true}},
            "start_cursor": "abc",
            "page_size": 25
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "results": [],
            "has_more": false,
            "next_cursor": null
        })))
        .mount(&server)
        .await;

    let client = make_client(&server);
    let req = QueryDataSourceRequest {
        filter: Some(json!({"property": "Done", "checkbox": {"equals": true}})),
        sorts: vec![],
        start_cursor: Some("abc".into()),
        page_size: Some(25),
    };
    let resp = client
        .query_data_source(&DataSourceId::parse(DS_ID_HEX).unwrap(), &req)
        .await
        .unwrap();
    assert_eq!(resp.results.len(), 0);
    assert!(resp.is_exhausted());
}

// === Page endpoints =======================================================

#[tokio::test]
async fn create_page_under_data_source() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/pages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_page_json(PAGE_ID_HEX)))
        .mount(&server)
        .await;

    let client = make_client(&server);
    let mut props = std::collections::HashMap::new();
    props.insert("Done".to_string(), PropertyValue::Checkbox { checkbox: true });
    let page = client
        .create_page(&CreatePageRequest {
            parent: PageParent::DataSource {
                data_source_id: DataSourceId::parse(DS_ID_HEX).unwrap(),
            },
            properties: props,
            children: vec![],
        })
        .await
        .unwrap();
    assert_eq!(page.id.as_str(), PAGE_ID_HEX);
}

#[tokio::test]
async fn update_page_properties() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/pages/{PAGE_ID_HEX}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_page_json(PAGE_ID_HEX)))
        .mount(&server)
        .await;

    let client = make_client(&server);
    let mut props = std::collections::HashMap::new();
    props.insert("Done".to_string(), PropertyValue::Checkbox { checkbox: false });
    let page = client
        .update_page(
            &PageId::parse(PAGE_ID_HEX).unwrap(),
            &UpdatePageRequest {
                properties: props,
                archived: None,
                in_trash: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(page.id.as_str(), PAGE_ID_HEX);
}

#[tokio::test]
async fn update_page_archive() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/pages/{PAGE_ID_HEX}")))
        .and(body_json(json!({"archived": true})))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_page_json(PAGE_ID_HEX)))
        .mount(&server)
        .await;

    let client = make_client(&server);
    let page = client
        .update_page(
            &PageId::parse(PAGE_ID_HEX).unwrap(),
            &UpdatePageRequest {
                properties: std::collections::HashMap::new(),
                archived: Some(true),
                in_trash: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(page.id.as_str(), PAGE_ID_HEX);
}

// === Rate limiter =========================================================

#[tokio::test]
async fn rate_limiter_paces_requests() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_page_json(PAGE_ID_HEX)))
        .mount(&server)
        .await;

    let config = ClientConfig {
        base_url: server.uri(),
        connect_timeout: Duration::from_secs(5),
        total_timeout: Duration::from_secs(10),
        max_response_bytes: notion_cli::api::MAX_RESPONSE_BYTES,
        rate_limit_per_sec: NonZeroU32::new(3).unwrap(),
    };
    let client = NotionClient::with_config(&NotionToken::new(TEST_TOKEN), config).unwrap();
    let page_id = PageId::parse(PAGE_ID_HEX).unwrap();

    // Warm up the bucket then measure.
    let _ = client.retrieve_page(&page_id).await;
    let _ = client.retrieve_page(&page_id).await;
    let _ = client.retrieve_page(&page_id).await;

    let start = std::time::Instant::now();
    for _ in 0..3 {
        let _ = client.retrieve_page(&page_id).await;
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(600),
        "3 req/s means 3 more requests take ~1s after bucket drain; elapsed={elapsed:?}",
    );
}
