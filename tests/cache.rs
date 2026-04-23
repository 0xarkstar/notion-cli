//! Cache module tests.

use std::num::NonZeroU32;
use std::time::Duration;

use notion_cli::api::page::UpdatePageRequest;
use notion_cli::api::{ClientConfig, NotionClient};
use notion_cli::cache::{Cache, LruCache};
use notion_cli::config::NotionToken;
use notion_cli::validation::PageId;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_TOKEN: &str = "ntn_test_cache_abcdef0123456789";
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
        "parent": {"type": "workspace", "workspace": true},
        "properties": {}
    })
}

fn make_cached_client(server: &MockServer, ttl_secs: u64) -> NotionClient {
    let config = ClientConfig {
        base_url: server.uri(),
        connect_timeout: Duration::from_secs(5),
        total_timeout: Duration::from_secs(10),
        max_response_bytes: notion_cli::api::MAX_RESPONSE_BYTES,
        rate_limit_per_sec: NonZeroU32::new(100).unwrap(),
        cache_ttl: Some(Duration::from_secs(ttl_secs)),
    };
    NotionClient::with_config(&NotionToken::new(TEST_TOKEN), config).unwrap()
}


// --- Unit tests for LruCache -----------------------------------------------

#[test]
fn lru_cache_stores_and_retrieves() {
    let cache = LruCache::with_ttl(Duration::from_secs(60));
    let val = json!({"key": "value"});
    cache.put("/pages/abc".to_string(), val.clone());
    assert_eq!(cache.get("/pages/abc"), Some(val));
}

#[test]
fn lru_cache_evicts_over_capacity() {
    // capacity=2, insert 3 entries → first entry evicted
    let cache = LruCache::new(2, Duration::from_secs(60));
    cache.put("a".to_string(), json!(1));
    cache.put("b".to_string(), json!(2));
    cache.put("c".to_string(), json!(3)); // evicts "a" (LRU)
    // "a" should be gone; "b" and "c" remain
    assert!(cache.get("a").is_none(), "evicted entry must be gone");
    assert_eq!(cache.get("b"), Some(json!(2)));
    assert_eq!(cache.get("c"), Some(json!(3)));
}

#[test]
fn lru_cache_ttl_expires_entry() {
    let cache = LruCache::with_ttl(Duration::from_millis(50));
    cache.put("key".to_string(), json!("value"));
    assert!(cache.get("key").is_some(), "should be present immediately");
    std::thread::sleep(Duration::from_millis(100));
    assert!(cache.get("key").is_none(), "should be expired after TTL");
}

#[test]
fn lru_cache_invalidate_prefix_matches() {
    let cache = LruCache::with_ttl(Duration::from_secs(60));
    cache.put("/pages/abc".to_string(), json!("page_abc"));
    cache.put("/pages/def".to_string(), json!("page_def"));
    cache.put("/databases/xyz".to_string(), json!("db_xyz"));

    cache.invalidate_prefix("/pages/");
    assert!(cache.get("/pages/abc").is_none());
    assert!(cache.get("/pages/def").is_none());
    // Unrelated key must survive.
    assert_eq!(cache.get("/databases/xyz"), Some(json!("db_xyz")));
}

// --- Integration tests against wiremock ------------------------------------

#[tokio::test]
async fn client_get_hits_cache_on_second_call() {
    let server = MockServer::start().await;
    // Mount mock for exactly ONE hit — second call must use cache.
    Mock::given(method("GET"))
        .and(path(format!("/v1/pages/{PAGE_ID_HEX}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_page_json(PAGE_ID_HEX)))
        .expect(1) // strictly one wire hit
        .mount(&server)
        .await;

    let client = make_cached_client(&server, 60);
    let id = PageId::parse(PAGE_ID_HEX).unwrap();
    let p1 = client.retrieve_page(&id).await.unwrap();
    let p2 = client.retrieve_page(&id).await.unwrap();
    assert_eq!(p1.id.as_str(), PAGE_ID_HEX);
    assert_eq!(p2.id.as_str(), PAGE_ID_HEX);
    // wiremock will assert exactly 1 request on drop
}

#[tokio::test]
async fn client_write_invalidates_cache_for_entity() {
    let server = MockServer::start().await;
    let page_path = format!("/v1/pages/{PAGE_ID_HEX}");

    // GET is expected twice (before and after PATCH).
    Mock::given(method("GET"))
        .and(path(page_path.clone()))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_page_json(PAGE_ID_HEX)))
        .expect(2)
        .mount(&server)
        .await;

    Mock::given(method("PATCH"))
        .and(path(page_path.clone()))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_page_json(PAGE_ID_HEX)))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_cached_client(&server, 60);
    let id = PageId::parse(PAGE_ID_HEX).unwrap();

    // First GET — populates cache.
    let _ = client.retrieve_page(&id).await.unwrap();

    // PATCH — should invalidate.
    let _ = client
        .update_page(
            &id,
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

    // Second GET — must hit the wire again (cache invalidated).
    let _ = client.retrieve_page(&id).await.unwrap();
    // wiremock enforces expect(2) GET and expect(1) PATCH on drop
}
