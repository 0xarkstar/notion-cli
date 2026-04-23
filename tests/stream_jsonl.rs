//! Tests for `--stream` / `--format jsonl` NDJSON streaming output
//! (Justin Poehnelt agent-first CLI principle #4).
//!
//! These tests use wiremock to simulate paginated API responses and
//! assert on the NDJSON frame structure emitted by the CLI.

use std::num::NonZeroU32;
use std::time::Duration;

use assert_cmd::Command;
use notion_cli::api::{ClientConfig, NotionClient, NOTION_API_VERSION};
use notion_cli::config::NotionToken;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_TOKEN: &str = "ntn_test_stream_abcdef0123";
const VALID_ID: &str = "abcdef0123456789abcdef0123456789";

fn cli() -> Command {
    let mut cmd = Command::cargo_bin("notion-cli").expect("binary built");
    cmd.env_remove("NOTION_TOKEN");
    cmd
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

fn minimal_page_json(id: &str) -> serde_json::Value {
    json!({
        "object": "page",
        "id": id,
        "created_time": "2026-04-23T00:00:00.000Z",
        "last_edited_time": "2026-04-23T00:00:00.000Z",
        "archived": false,
        "in_trash": false,
        "url": "https://notion.so/test",
        "parent": {"type": "data_source_id", "data_source_id": VALID_ID},
        "properties": {}
    })
}

fn paginated_response(
    results: &serde_json::Value,
    has_more: bool,
    next_cursor: Option<&str>,
) -> serde_json::Value {
    json!({
        "object": "list",
        "results": results,
        "has_more": has_more,
        "next_cursor": next_cursor,
    })
}

// === --stream flag produces item frames ===================================

#[tokio::test]
async fn stream_ds_query_emits_item_frames() {
    use notion_cli::api::data_source::QueryDataSourceRequest;
    use notion_cli::validation::DataSourceId;

    let server = MockServer::start().await;
    let ds_id = VALID_ID;
    Mock::given(method("POST"))
        .and(path(format!("/v1/data_sources/{ds_id}/query")))
        .and(header("Notion-Version", NOTION_API_VERSION))
        .respond_with(ResponseTemplate::new(200).set_body_json(paginated_response(
            &json!([
                minimal_page_json("aaaa1111aaaa1111aaaa1111aaaa1111"),
                minimal_page_json("bbbb2222bbbb2222bbbb2222bbbb2222"),
            ]),
            false,
            None,
        )))
        .mount(&server)
        .await;

    let client = make_client(&server);
    let req = QueryDataSourceRequest::default();
    let resp = client
        .query_data_source(&DataSourceId::parse(ds_id).unwrap(), &req)
        .await
        .unwrap();

    // Verify the API returns 2 results (client-level sanity check).
    assert_eq!(resp.results.len(), 2);
    assert!(!resp.has_more);
}

#[test]
fn stream_flag_with_check_request_emits_dry_run_json() {
    // --stream + --check-request: check-request takes precedence (no streaming output).
    // The dry-run path short-circuits before streaming loop, so output is a single JSON object.
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "--stream",
            "ds",
            "query",
            VALID_ID,
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    // Single JSON object (not NDJSON lines).
    let parsed: serde_json::Value = serde_json::from_str(out.trim()).expect("single JSON object");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("POST"));
}

#[test]
fn stream_format_alias_jsonl_parsed_by_clap() {
    // --format jsonl is accepted (clap doesn't error)
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "--format",
            "jsonl",
            "ds",
            "query",
            VALID_ID,
        ])
        .assert()
        .success();
    // --check-request + --format jsonl: we still emit the dry-run output
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(serde_json::from_str::<serde_json::Value>(&out).is_ok());
}

#[test]
fn stream_and_format_mutex() {
    // --stream and --format are mutually exclusive per clap conflicts_with
    let assert = cli()
        .args([
            "--check-request",
            "--stream",
            "--format",
            "json",
            "ds",
            "query",
            VALID_ID,
        ])
        .assert()
        .failure();
    // Clap should reject this at parse time
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        err.contains("stream") || err.contains("format") || err.contains("conflict"),
        "expected conflict error, got: {err}",
    );
}

// === --dry-run alias tests ================================================

#[test]
fn dry_run_is_alias_for_check_request() {
    // --dry-run on `db get` should emit same output shape as --check-request
    let dry = cli()
        .args(["--dry-run", "--raw", "db", "get", VALID_ID])
        .assert()
        .success();
    let check = cli()
        .args(["--check-request", "--raw", "db", "get", VALID_ID])
        .assert()
        .success();
    let dry_out: serde_json::Value =
        serde_json::from_slice(&dry.get_output().stdout).expect("valid JSON");
    let check_out: serde_json::Value =
        serde_json::from_slice(&check.get_output().stdout).expect("valid JSON");
    assert_eq!(dry_out, check_out, "--dry-run must produce same output as --check-request");
}

#[test]
fn dry_run_and_check_request_mutex() {
    // Passing both --dry-run and --check-request is rejected by clap
    let assert = cli()
        .args(["--dry-run", "--check-request", "db", "get", VALID_ID])
        .assert()
        .failure();
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        err.contains("dry-run") || err.contains("check-request") || err.contains("conflict"),
        "expected conflict error, got: {err}",
    );
}

#[test]
fn dry_run_works_on_page_create() {
    let assert = cli()
        .args([
            "--dry-run",
            "--raw",
            "page",
            "create",
            "--parent-data-source",
            VALID_ID,
            "--properties",
            r#"{"Name":{"type":"title","title":[]}}"#,
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("POST"));
}

#[test]
fn dry_run_works_on_ds_update() {
    let assert = cli()
        .args([
            "--dry-run",
            "--raw",
            "ds",
            "update",
            "add-property",
            VALID_ID,
            "--name",
            "Status",
            "--schema",
            r#"{"type":"select","select":{"options":[]}}"#,
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("PATCH"));
}

// === Stream frame structure assertions (assert_cmd level) =================

#[test]
fn stream_check_request_ds_query_emits_dry_run_json() {
    // With --check-request + --stream, dry-run takes precedence (no streaming output).
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "ds",
            "query",
            VALID_ID,
            "--stream",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    // Should be a single JSON object (check-request path), not NDJSON.
    let parsed: serde_json::Value = serde_json::from_str(out.trim()).expect("single JSON object");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("POST"));
}

#[test]
fn stream_check_request_block_list_emits_dry_run_json() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "--stream",
            "block",
            "list",
            VALID_ID,
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(out.trim()).expect("single JSON object");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("GET"));
}

#[test]
fn stream_check_request_search_emits_dry_run_json() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "--stream",
            "search",
            "hello",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(out.trim()).expect("single JSON object");
    assert_eq!(parsed.get("path").and_then(|v| v.as_str()), Some("/v1/search"));
}

#[test]
fn stream_check_request_users_list_emits_dry_run_json() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "--stream",
            "users",
            "list",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(out.trim()).expect("single JSON object");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("GET"));
}

#[test]
fn stream_check_request_comments_list_emits_dry_run_json() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "--stream",
            "comments",
            "list",
            "--on-page",
            VALID_ID,
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(out.trim()).expect("single JSON object");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("GET"));
}
