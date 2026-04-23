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
    AppendBlockChildrenParams, CreateDataSourceParams, CreatePageParams, DbUpdateParams,
    DeleteBlockParams, GetBlockParams, GetDataSourceParams, GetPageParams, ListBlockChildrenParams,
    QueryDataSourceParams, SearchParams, UpdateBlockParams, UpdatePageParams,
};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DB_ID: &str = "abcdef0123456789abcdef0123456789";
const DS_ID: &str = "fedcba9876543210fedcba9876543210";
const PAGE_ID: &str = "11111111111111111111111111111111";
const BLOCK_ID: &str = "22222222222222222222222222222222";

fn client(server: &MockServer) -> NotionClient {
    NotionClient::with_config(
        &NotionToken::new("ntn_test"),
        ClientConfig {
            base_url: server.uri(),
            connect_timeout: Duration::from_secs(5),
            total_timeout: Duration::from_secs(10),
            max_response_bytes: 10 * 1024 * 1024,
            rate_limit_per_sec: NonZeroU32::new(100).unwrap(), cache_ttl: None,
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
            children: None,
            icon: None,
            cover: None,
            idempotency_key: None,
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
            children: None,
            icon: None,
            cover: None,
            idempotency_key: None,
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
            children: None,
            icon: None,
            cover: None,
            idempotency_key: None,
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
            children: None,
            icon: None,
            cover: None,
            idempotency_key: None,
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
            icon: None,
            cover: None,
            idempotency_key: None,
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
            idempotency_key: None,
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
            idempotency_key: None,
        },
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("parent_database_id"), "got {err:?}");
}

// === Block handlers =======================================================

fn block_response(id: &str, text: &str) -> serde_json::Value {
    json!({
        "object": "block",
        "id": id,
        "created_time": "2026-04-17T10:00:00.000Z",
        "last_edited_time": "2026-04-17T10:00:00.000Z",
        "has_children": false,
        "archived": false,
        "in_trash": false,
        "type": "paragraph",
        "paragraph": {
            "rich_text": [{"type":"text","text":{"content": text},"annotations":{"bold":false,"italic":false,"strikethrough":false,"underline":false,"code":false,"color":"default"},"plain_text": text}],
            "color": "default"
        }
    })
}

#[tokio::test]
async fn handler_get_block() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/blocks/{BLOCK_ID}")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(block_response(BLOCK_ID, "hi")),
        )
        .mount(&server)
        .await;
    let c = client(&server);
    let out = notion_cli::mcp::handlers::get_block(
        &c,
        GetBlockParams { block_id: BLOCK_ID.into() },
    )
    .await
    .unwrap();
    assert_envelope(&out);
}

#[tokio::test]
async fn handler_get_block_rejects_invalid_id() {
    let server = MockServer::start().await;
    let c = client(&server);
    let err = notion_cli::mcp::handlers::get_block(
        &c,
        GetBlockParams { block_id: "not-an-id".into() },
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("block_id"), "got {err:?}");
}

#[tokio::test]
async fn handler_list_block_children() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/blocks/{BLOCK_ID}/children")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "results": [block_response(BLOCK_ID, "a")],
            "has_more": false,
            "next_cursor": null
        })))
        .mount(&server)
        .await;
    let c = client(&server);
    let out = notion_cli::mcp::handlers::list_block_children(
        &c,
        ListBlockChildrenParams {
            block_id: BLOCK_ID.into(),
            start_cursor: None,
            page_size: Some(5),
        },
    )
    .await
    .unwrap();
    assert_envelope(&out);
}

#[tokio::test]
async fn handler_append_block_children() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/blocks/{BLOCK_ID}/children")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "results": [],
            "has_more": false,
            "next_cursor": null
        })))
        .mount(&server)
        .await;
    let c = client(&server);
    let out = notion_cli::mcp::handlers::append_block_children(
        &c,
        AppendBlockChildrenParams {
            block_id: BLOCK_ID.into(),
            children: json!([
                {"type":"paragraph","paragraph":{"rich_text":[],"color":"default"}}
            ]),
            after: None,
            idempotency_key: None,
        },
    )
    .await
    .unwrap();
    assert_envelope(&out);
}

#[tokio::test]
async fn handler_append_block_children_rejects_malformed_children() {
    let server = MockServer::start().await;
    let c = client(&server);
    let err = notion_cli::mcp::handlers::append_block_children(
        &c,
        AppendBlockChildrenParams {
            block_id: BLOCK_ID.into(),
            children: json!({"not": "an array"}),
            after: None,
            idempotency_key: None,
        },
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("children"), "got {err:?}");
}

#[tokio::test]
async fn handler_update_block_archive() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/blocks/{BLOCK_ID}")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(block_response(BLOCK_ID, "x")),
        )
        .mount(&server)
        .await;
    let c = client(&server);
    let out = notion_cli::mcp::handlers::update_block(
        &c,
        UpdateBlockParams {
            block_id: BLOCK_ID.into(),
            body: None,
            archived: Some(true),
            in_trash: None,
            idempotency_key: None,
        },
    )
    .await
    .unwrap();
    assert_envelope(&out);
}

#[tokio::test]
async fn handler_delete_block() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path(format!("/v1/blocks/{BLOCK_ID}")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(block_response(BLOCK_ID, "gone")),
        )
        .mount(&server)
        .await;
    let c = client(&server);
    let out = notion_cli::mcp::handlers::delete_block(
        &c,
        DeleteBlockParams { block_id: BLOCK_ID.into(), idempotency_key: None },
    )
    .await
    .unwrap();
    assert_envelope(&out);
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

// === v0.4 handlers ========================================================

#[tokio::test]
async fn db_update_handler_validates_parent_mutex() {
    let server = MockServer::start().await;
    let c = client(&server);
    let err = handlers::db_update(
        &c,
        DbUpdateParams {
            database_id: DB_ID.into(),
            to_page_id: Some(PAGE_ID.into()),
            to_workspace: Some(true),
            title: None,
            description: None,
            icon: None,
            cover: None,
            is_inline: None,
            is_locked: None,
            in_trash: None,
            idempotency_key: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.message.contains("mutually exclusive"),
        "expected mutex error, got: {err:?}",
    );
}

#[tokio::test]
async fn db_update_handler_rejects_empty_req() {
    let server = MockServer::start().await;
    let c = client(&server);
    let err = handlers::db_update(
        &c,
        DbUpdateParams {
            database_id: DB_ID.into(),
            to_page_id: None,
            to_workspace: None,
            title: None,
            description: None,
            icon: None,
            cover: None,
            is_inline: None,
            is_locked: None,
            in_trash: None,
            idempotency_key: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.message.contains("at least one field"),
        "expected empty-request error, got: {err:?}",
    );
}

#[tokio::test]
async fn db_update_handler_icon_tristate_null_clears() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/databases/{DB_ID}")))
        .and(wiremock::matchers::body_json(serde_json::json!({"icon": null})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": "database",
            "id": DB_ID,
            "created_time": "2026-04-22T10:00:00.000Z",
            "last_edited_time": "2026-04-22T10:00:00.000Z",
            "title": [],
            "description": [],
            "archived": false,
            "in_trash": false,
            "properties": {},
            "data_sources": []
        })))
        .mount(&server)
        .await;
    let c = client(&server);
    let out = handlers::db_update(
        &c,
        DbUpdateParams {
            database_id: DB_ID.into(),
            to_page_id: None,
            to_workspace: None,
            title: None,
            description: None,
            icon: Some(serde_json::Value::Null), // tristate clear
            cover: None,
            is_inline: None,
            is_locked: None,
            in_trash: None,
            idempotency_key: None,
        },
    )
    .await
    .unwrap();
    assert_envelope(&out);
}

#[tokio::test]
async fn users_me_handler_happy_path() {
    let bot_hex = "aaaabbbbccccddddaaaabbbbccccdddd";
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": "user",
            "id": bot_hex,
            "type": "bot",
            "bot": {"owner": {"type": "workspace", "workspace": true}, "workspace_name": "Test"},
            "name": "My Bot",
            "avatar_url": null
        })))
        .mount(&server)
        .await;
    let c = client(&server);
    let out = handlers::users_me(&c).await.unwrap();
    assert_envelope(&out);
    assert_eq!(
        out.pointer("/content/id").and_then(|v| v.as_str()),
        Some(bot_hex),
    );
}
