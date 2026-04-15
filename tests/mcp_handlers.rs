//! Direct handler tests for `src/mcp/handlers.rs` — bypass the rmcp
//! stdio layer and exercise the validate → client-call → envelope
//! flow against wiremock. This is the highest-ROI coverage gap:
//! MCP handlers are the translation layer between tool params and
//! API calls, and all write paths live here.

use std::num::NonZeroU32;
use std::time::Duration;

use notion_cli::api::{ClientConfig, NotionClient};
use notion_cli::config::NotionToken;
use notion_cli::mcp::handlers;
use notion_cli::mcp::params::{
    CreateDataSourceParams, CreatePageParams, GetDataSourceParams, GetPageParams,
    QueryDataSourceParams, SearchParams, UpdatePageParams,
};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DB_ID: &str = "abcdef0123456789abcdef0123456789";
const DS_ID: &str = "fedcba9876543210fedcba9876543210";
const PAGE_ID: &str = "11111111111111111111111111111111";

fn client(server: &MockServer) -> NotionClient {
    NotionClient::with_config(
        &NotionToken::new("ntn_test"),
        ClientConfig {
            base_url: server.uri(),
            connect_timeout: Duration::from_secs(5),
            total_timeout: Duration::from_secs(10),
            max_response_bytes: 10 * 1024 * 1024,
            rate_limit_per_sec: NonZeroU32::new(100).unwrap(),
        },
    )
    .unwrap()
}

fn page_response(id: &str) -> serde_json::Value {
    json!({
        "object": "page",
        "id": id,
        "created_time": "2026-04-15T10:00:00.000Z",
        "last_edited_time": "2026-04-15T10:00:00.000Z",
        "archived": false,
        "in_trash": false,
        "url": "https://notion.so/p",
        "parent": {"type": "data_source_id", "data_source_id": DS_ID},
        "properties": {}
    })
}

fn data_source_response(id: &str) -> serde_json::Value {
    json!({
        "object": "data_source",
        "id": id,
        "created_time": "2026-04-15T10:00:00.000Z",
        "last_edited_time": "2026-04-15T10:00:00.000Z",
        "name": "Test",
        "properties": {}
    })
}

fn assert_envelope(v: &serde_json::Value) {
    assert_eq!(v.get("source").and_then(|x| x.as_str()), Some("notion"));
    assert_eq!(v.get("trust").and_then(|x| x.as_str()), Some("untrusted"));
    assert!(v.get("api_version").is_some());
    assert!(v.get("content").is_some());
}

// === Read handlers ========================================================

#[tokio::test]
async fn handler_get_page_wraps_in_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/pages/{PAGE_ID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(page_response(PAGE_ID)))
        .mount(&server)
        .await;
    let c = client(&server);
    let out = handlers::get_page(&c, GetPageParams { page_id: PAGE_ID.into() })
        .await
        .unwrap();
    assert_envelope(&out);
    assert_eq!(
        out.pointer("/content/id").and_then(|v| v.as_str()),
        Some(PAGE_ID),
    );
}

#[tokio::test]
async fn handler_get_page_accepts_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/pages/{PAGE_ID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(page_response(PAGE_ID)))
        .mount(&server)
        .await;
    let c = client(&server);
    let url = format!("https://notion.so/Page-{PAGE_ID}");
    let out = handlers::get_page(&c, GetPageParams { page_id: url }).await.unwrap();
    assert_envelope(&out);
}

#[tokio::test]
async fn handler_get_page_rejects_invalid_id() {
    let server = MockServer::start().await;
    let c = client(&server);
    let err = handlers::get_page(
        &c,
        GetPageParams { page_id: "not-an-id".into() },
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("page_id"), "got {err:?}");
}

#[tokio::test]
async fn handler_get_data_source() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/data_sources/{DS_ID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(data_source_response(DS_ID)))
        .mount(&server)
        .await;
    let c = client(&server);
    let out = handlers::get_data_source(
        &c,
        GetDataSourceParams { data_source_id: DS_ID.into() },
    )
    .await
    .unwrap();
    assert_envelope(&out);
}

#[tokio::test]
async fn handler_query_data_source_with_filter() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/data_sources/{DS_ID}/query")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "results": [page_response(PAGE_ID)],
            "has_more": false,
            "next_cursor": null
        })))
        .mount(&server)
        .await;
    let c = client(&server);
    let out = handlers::query_data_source(
        &c,
        QueryDataSourceParams {
            data_source_id: DS_ID.into(),
            filter: Some(json!({"property": "Done", "checkbox": {"equals": true}})),
            sorts: Some(json!([{"property": "Name", "direction": "ascending"}])),
            start_cursor: None,
            page_size: Some(10),
        },
    )
    .await
    .unwrap();
    assert_envelope(&out);
    assert_eq!(
        out.pointer("/content/results/0/id").and_then(|v| v.as_str()),
        Some(PAGE_ID),
    );
}

#[tokio::test]
async fn handler_search() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "results": [],
            "has_more": false,
            "next_cursor": null
        })))
        .mount(&server)
        .await;
    let c = client(&server);
    let out = handlers::search(
        &c,
        SearchParams {
            query: Some("hello".into()),
            filter: None,
            sort: None,
            start_cursor: None,
            page_size: Some(5),
        },
    )
    .await
    .unwrap();
    assert_envelope(&out);
}

// === Write handlers =======================================================

#[tokio::test]
async fn handler_create_page_data_source_parent() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/pages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page_response(PAGE_ID)))
        .mount(&server)
        .await;
    let c = client(&server);
    let out = handlers::create_page(
        &c,
        CreatePageParams {
            parent_data_source_id: Some(DS_ID.into()),
            parent_page_id: None,
            properties: json!({"Done": {"type": "checkbox", "checkbox": true}}),
        },
    )
    .await
    .unwrap();
    assert_envelope(&out);
}

#[tokio::test]
async fn handler_create_page_page_parent() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/pages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page_response(PAGE_ID)))
        .mount(&server)
        .await;
    let c = client(&server);
    let out = handlers::create_page(
        &c,
        CreatePageParams {
            parent_data_source_id: None,
            parent_page_id: Some(PAGE_ID.into()),
            properties: json!({}),
        },
    )
    .await
    .unwrap();
    assert_envelope(&out);
}

#[tokio::test]
async fn handler_create_page_rejects_both_parents() {
    let server = MockServer::start().await;
    let c = client(&server);
    let err = handlers::create_page(
        &c,
        CreatePageParams {
            parent_data_source_id: Some(DS_ID.into()),
            parent_page_id: Some(PAGE_ID.into()),
            properties: json!({}),
        },
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("exactly one"), "got {err:?}");
}

#[tokio::test]
async fn handler_create_page_rejects_no_parent() {
    let server = MockServer::start().await;
    let c = client(&server);
    let err = handlers::create_page(
        &c,
        CreatePageParams {
            parent_data_source_id: None,
            parent_page_id: None,
            properties: json!({}),
        },
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("exactly one"), "got {err:?}");
}

#[tokio::test]
async fn handler_update_page() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/pages/{PAGE_ID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(page_response(PAGE_ID)))
        .mount(&server)
        .await;
    let c = client(&server);
    let out = handlers::update_page(
        &c,
        UpdatePageParams {
            page_id: PAGE_ID.into(),
            properties: Some(json!({"Done": {"type": "checkbox", "checkbox": false}})),
            archived: None,
            in_trash: Some(true),
        },
    )
    .await
    .unwrap();
    assert_envelope(&out);
}

#[tokio::test]
async fn handler_create_data_source_the_bug_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/data_sources"))
        .respond_with(ResponseTemplate::new(200).set_body_json(data_source_response(DS_ID)))
        .mount(&server)
        .await;
    let c = client(&server);
    let out = handlers::create_data_source(
        &c,
        CreateDataSourceParams {
            parent_database_id: DB_ID.into(),
            title: Some("new-ds".into()),
            properties: json!({"Name": {"title": {}}}),
        },
    )
    .await
    .unwrap();
    assert_envelope(&out);
    assert_eq!(
        out.pointer("/content/id").and_then(|v| v.as_str()),
        Some(DS_ID),
    );
}

#[tokio::test]
async fn handler_create_data_source_rejects_bad_parent() {
    let server = MockServer::start().await;
    let c = client(&server);
    let err = handlers::create_data_source(
        &c,
        CreateDataSourceParams {
            parent_database_id: "not-an-id".into(),
            title: None,
            properties: json!({}),
        },
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("parent_database_id"), "got {err:?}");
}

// === Error mapping =======================================================

#[tokio::test]
async fn handler_maps_api_404_to_rpc_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(json!({
                "object": "error",
                "code": "object_not_found",
                "message": "Page not found"
            })),
        )
        .mount(&server)
        .await;
    let c = client(&server);
    let err = handlers::get_page(
        &c,
        GetPageParams { page_id: PAGE_ID.into() },
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("not found") || err.message.contains("resource"));
}

#[tokio::test]
async fn handler_maps_api_500_to_internal_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let c = client(&server);
    let err = handlers::get_page(
        &c,
        GetPageParams { page_id: PAGE_ID.into() },
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("500") || err.message.to_ascii_lowercase().contains("server"));
}
