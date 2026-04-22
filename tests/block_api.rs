//! Wiremock tests for `/v1/blocks/*` endpoints.

use std::num::NonZeroU32;
use std::time::Duration;

use notion_cli::api::block::{AppendBlockChildrenRequest, UpdateBlockRequest};
use notion_cli::api::{ClientConfig, NotionClient, NOTION_API_VERSION};
use notion_cli::config::NotionToken;
use notion_cli::types::block::{Block, BlockBody};
use notion_cli::validation::BlockId;
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_TOKEN: &str = "ntn_test_abcdef0123456789";
const BLOCK_ID: &str = "11111111111111111111111111111111";
const PARENT_PAGE: &str = "22222222222222222222222222222222";

fn make_client(server: &MockServer) -> NotionClient {
    let config = ClientConfig {
        base_url: server.uri(),
        connect_timeout: Duration::from_secs(5),
        total_timeout: Duration::from_secs(10),
        max_response_bytes: notion_cli::api::MAX_RESPONSE_BYTES,
        rate_limit_per_sec: NonZeroU32::new(100).unwrap(),
    };
    NotionClient::with_config(&NotionToken::new(TEST_TOKEN), config).unwrap()
}

fn paragraph_block_json(id: &str, text: &str) -> serde_json::Value {
    json!({
        "object": "block",
        "id": id,
        "parent": {"type": "page_id", "page_id": PARENT_PAGE},
        "created_time": "2026-04-17T10:00:00.000Z",
        "last_edited_time": "2026-04-17T10:00:00.000Z",
        "has_children": false,
        "archived": false,
        "in_trash": false,
        "type": "paragraph",
        "paragraph": {
            "rich_text": [{
                "type": "text",
                "text": {"content": text},
                "annotations": {"bold": false, "italic": false, "strikethrough": false, "underline": false, "code": false, "color": "default"},
                "plain_text": text
            }],
            "color": "default"
        }
    })
}

// === Retrieve =============================================================

#[tokio::test]
async fn retrieve_block() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/blocks/{BLOCK_ID}")))
        .and(header("Notion-Version", NOTION_API_VERSION))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(paragraph_block_json(BLOCK_ID, "Hello")),
        )
        .mount(&server)
        .await;
    let client = make_client(&server);
    let block = client
        .retrieve_block(&BlockId::parse(BLOCK_ID).unwrap())
        .await
        .unwrap();
    let typed = block.as_known().expect("known");
    assert_eq!(typed.id.as_str(), BLOCK_ID);
    assert!(matches!(&typed.body, BlockBody::Paragraph { .. }));
}

#[tokio::test]
async fn retrieve_unknown_type_falls_through_to_raw() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({
                "object": "block",
                "id": BLOCK_ID,
                "created_time": "2026-04-17T10:00:00.000Z",
                "last_edited_time": "2026-04-17T10:00:00.000Z",
                "has_children": false,
                "archived": false,
                "in_trash": false,
                "type": "synced_block",
                "synced_block": {"synced_from": null}
            })),
        )
        .mount(&server)
        .await;
    let client = make_client(&server);
    let block = client
        .retrieve_block(&BlockId::parse(BLOCK_ID).unwrap())
        .await
        .unwrap();
    assert!(matches!(block, Block::Raw(_)));
}

// === List children ========================================================

#[tokio::test]
async fn list_block_children_paginated() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/blocks/{BLOCK_ID}/children")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "results": [
                paragraph_block_json("aaaa1111aaaa1111aaaa1111aaaa1111", "first"),
                paragraph_block_json("bbbb2222bbbb2222bbbb2222bbbb2222", "second"),
            ],
            "has_more": true,
            "next_cursor": "next-cursor-abc"
        })))
        .mount(&server)
        .await;
    let client = make_client(&server);
    let resp = client
        .list_block_children(&BlockId::parse(BLOCK_ID).unwrap(), None, Some(2))
        .await
        .unwrap();
    assert_eq!(resp.results.len(), 2);
    assert!(resp.has_more);
    assert_eq!(resp.next_cursor.as_deref(), Some("next-cursor-abc"));
}

#[tokio::test]
async fn list_block_children_cursor_in_query_string() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/blocks/{BLOCK_ID}/children")))
        .and(wiremock::matchers::query_param("start_cursor", "abc123"))
        .and(wiremock::matchers::query_param("page_size", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "results": [],
            "has_more": false,
            "next_cursor": null
        })))
        .mount(&server)
        .await;
    let client = make_client(&server);
    let resp = client
        .list_block_children(
            &BlockId::parse(BLOCK_ID).unwrap(),
            Some("abc123"),
            Some(5),
        )
        .await
        .unwrap();
    assert!(resp.is_exhausted());
}

// === Append children ======================================================

#[tokio::test]
async fn append_block_children_sends_typed_bodies() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/blocks/{BLOCK_ID}/children")))
        .and(body_json(json!({
            "children": [
                {"type": "paragraph", "paragraph": {"rich_text": [{"type":"text","text":{"content":"hello"},"annotations":{"bold":false,"italic":false,"strikethrough":false,"underline":false,"code":false,"color":"default"},"plain_text":"hello"}], "color": "default"}},
                {"type": "heading_1", "heading_1": {"rich_text": [{"type":"text","text":{"content":"Title"},"annotations":{"bold":false,"italic":false,"strikethrough":false,"underline":false,"code":false,"color":"default"},"plain_text":"Title"}], "color": "default", "is_toggleable": false}},
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "results": [
                paragraph_block_json("cccc3333cccc3333cccc3333cccc3333", "hello"),
            ],
            "has_more": false,
            "next_cursor": null
        })))
        .mount(&server)
        .await;
    let client = make_client(&server);
    let req = AppendBlockChildrenRequest::new(vec![
        BlockBody::paragraph("hello"),
        BlockBody::heading_1("Title"),
    ]);
    let resp = client
        .append_block_children(&BlockId::parse(BLOCK_ID).unwrap(), &req)
        .await
        .unwrap();
    assert_eq!(resp.results.len(), 1);
}

#[tokio::test]
async fn append_block_children_empty_list_ok() {
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
    let client = make_client(&server);
    let req = AppendBlockChildrenRequest::new(vec![]);
    let resp = client
        .append_block_children(&BlockId::parse(BLOCK_ID).unwrap(), &req)
        .await
        .unwrap();
    assert_eq!(resp.results.len(), 0);
}

// === Update ==============================================================

#[tokio::test]
async fn update_block_content() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/blocks/{BLOCK_ID}")))
        .and(body_json(json!({
            "type": "paragraph",
            "paragraph": {"rich_text": [{"type":"text","text":{"content":"updated"},"annotations":{"bold":false,"italic":false,"strikethrough":false,"underline":false,"code":false,"color":"default"},"plain_text":"updated"}], "color": "default"}
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(paragraph_block_json(BLOCK_ID, "updated")),
        )
        .mount(&server)
        .await;
    let client = make_client(&server);
    let req = UpdateBlockRequest {
        body: Some(BlockBody::paragraph("updated")),
        archived: None,
        in_trash: None,
    };
    let block = client
        .update_block(&BlockId::parse(BLOCK_ID).unwrap(), &req)
        .await
        .unwrap();
    assert!(block.is_writable());
}

#[tokio::test]
async fn update_block_body_plus_archived_combined() {
    // Tests the flatten + explicit field combination. body's fields
    // flatten to top level; archived sits alongside.
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/blocks/{BLOCK_ID}")))
        .and(body_json(json!({
            "type": "paragraph",
            "paragraph": {"rich_text": [{"type":"text","text":{"content":"done"},"annotations":{"bold":false,"italic":false,"strikethrough":false,"underline":false,"code":false,"color":"default"},"plain_text":"done"}], "color": "default"},
            "archived": true
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(paragraph_block_json(BLOCK_ID, "done")),
        )
        .mount(&server)
        .await;
    let client = make_client(&server);
    let req = UpdateBlockRequest {
        body: Some(BlockBody::paragraph("done")),
        archived: Some(true),
        in_trash: None,
    };
    client
        .update_block(&BlockId::parse(BLOCK_ID).unwrap(), &req)
        .await
        .unwrap();
}

#[tokio::test]
async fn list_block_children_url_encodes_cursor() {
    let server = MockServer::start().await;
    // Cursor with reserved URL characters — pagination must still work.
    Mock::given(method("GET"))
        .and(path(format!("/v1/blocks/{BLOCK_ID}/children")))
        .and(wiremock::matchers::query_param("start_cursor", "a+b=c&d"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "results": [],
            "has_more": false,
            "next_cursor": null
        })))
        .mount(&server)
        .await;
    let client = make_client(&server);
    client
        .list_block_children(&BlockId::parse(BLOCK_ID).unwrap(), Some("a+b=c&d"), None)
        .await
        .unwrap();
}

#[tokio::test]
async fn update_block_archive_only() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/blocks/{BLOCK_ID}")))
        .and(body_json(json!({"archived": true})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(paragraph_block_json(BLOCK_ID, "x")),
        )
        .mount(&server)
        .await;
    let client = make_client(&server);
    let req = UpdateBlockRequest {
        body: None,
        archived: Some(true),
        in_trash: None,
    };
    client
        .update_block(&BlockId::parse(BLOCK_ID).unwrap(), &req)
        .await
        .unwrap();
}

// === Delete ==============================================================

#[tokio::test]
async fn delete_block() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path(format!("/v1/blocks/{BLOCK_ID}")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(paragraph_block_json(BLOCK_ID, "gone")),
        )
        .mount(&server)
        .await;
    let client = make_client(&server);
    let block = client
        .delete_block(&BlockId::parse(BLOCK_ID).unwrap())
        .await
        .unwrap();
    assert!(block.is_writable());
}

// === page create with children ===========================================

#[tokio::test]
async fn create_page_with_children_inline() {
    use notion_cli::api::page::{CreatePageRequest, PageParent};
    use notion_cli::types::property::PropertyValue;
    use notion_cli::validation::DataSourceId;

    let server = MockServer::start().await;
    let ds_id = "dddddddddddddddddddddddddddddddd";
    Mock::given(method("POST"))
        .and(path("/v1/pages"))
        .and(body_json(json!({
            "parent": {"type": "data_source_id", "data_source_id": ds_id},
            "properties": {
                "Name": {"type": "title", "title": [{"type":"text","text":{"content":"Page"},"annotations":{"bold":false,"italic":false,"strikethrough":false,"underline":false,"code":false,"color":"default"},"plain_text":"Page"}]}
            },
            "children": [
                {"type": "heading_1", "heading_1": {"rich_text": [{"type":"text","text":{"content":"H1"},"annotations":{"bold":false,"italic":false,"strikethrough":false,"underline":false,"code":false,"color":"default"},"plain_text":"H1"}], "color": "default", "is_toggleable": false}},
                {"type": "paragraph", "paragraph": {"rich_text": [{"type":"text","text":{"content":"body"},"annotations":{"bold":false,"italic":false,"strikethrough":false,"underline":false,"code":false,"color":"default"},"plain_text":"body"}], "color": "default"}},
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "page",
            "id": PARENT_PAGE,
            "created_time": "2026-04-17T10:00:00.000Z",
            "last_edited_time": "2026-04-17T10:00:00.000Z",
            "archived": false,
            "in_trash": false,
            "url": "https://notion.so/...",
            "parent": {"type": "data_source_id", "data_source_id": ds_id},
            "properties": {}
        })))
        .mount(&server)
        .await;
    let client = make_client(&server);
    let mut props = std::collections::HashMap::new();
    props.insert(
        "Name".to_string(),
        PropertyValue::Title {
            title: vec![notion_cli::types::rich_text::RichText {
                content: notion_cli::types::rich_text::RichTextContent::Text {
                    text: notion_cli::types::rich_text::TextContent {
                        content: "Page".into(),
                        link: None,
                    },
                },
                annotations: notion_cli::types::rich_text::Annotations::default(),
                plain_text: "Page".into(),
                href: None,
            }],
        },
    );
    let req = CreatePageRequest {
        parent: PageParent::DataSource {
            data_source_id: DataSourceId::parse(ds_id).unwrap(),
        },
        properties: props,
        children: vec![BlockBody::heading_1("H1"), BlockBody::paragraph("body")],
        icon: None,
        cover: None,
    };
    let page = client.create_page(&req).await.unwrap();
    assert_eq!(page.id.as_str(), PARENT_PAGE);
}

#[tokio::test]
async fn create_page_omits_empty_children() {
    use notion_cli::api::page::{CreatePageRequest, PageParent};
    use notion_cli::validation::DataSourceId;

    let server = MockServer::start().await;
    let ds_id = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    // Body must NOT include `children` when the list is empty.
    Mock::given(method("POST"))
        .and(path("/v1/pages"))
        .and(body_json(json!({
            "parent": {"type": "data_source_id", "data_source_id": ds_id},
            "properties": {}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "page",
            "id": PARENT_PAGE,
            "created_time": "2026-04-17T10:00:00.000Z",
            "last_edited_time": "2026-04-17T10:00:00.000Z",
            "archived": false,
            "in_trash": false,
            "url": "https://notion.so/",
            "parent": {"type": "data_source_id", "data_source_id": ds_id},
            "properties": {}
        })))
        .mount(&server)
        .await;
    let client = make_client(&server);
    let req = CreatePageRequest {
        parent: PageParent::DataSource {
            data_source_id: DataSourceId::parse(ds_id).unwrap(),
        },
        properties: std::collections::HashMap::new(),
        children: vec![],
        icon: None,
        cover: None,
    };
    client.create_page(&req).await.unwrap();
}
