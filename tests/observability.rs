//! Observability unit tests — request IDs, audit schema v2, cost estimates.

use std::fs;
use std::path::PathBuf;

use notion_cli::mcp::audit::AuditLog;
use notion_cli::observability::cost::CostEstimate;
use notion_cli::observability::RequestId;

fn tmp_path(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "notion-cli-obs-{}-{name}.jsonl",
        std::process::id()
    ));
    let _ = fs::remove_file(&p);
    p
}

// === RequestId ============================================================

#[test]
fn request_id_new_is_uuid_v7() {
    let id = RequestId::new();
    let parsed = uuid::Uuid::parse_str(id.as_str()).expect("valid UUID");
    assert_eq!(parsed.get_version_num(), 7, "expected UUID version 7");
}

#[test]
fn request_id_monotonic() {
    let a = RequestId::new();
    // Spin briefly to ensure distinct milliseconds in the v7 timestamp.
    std::thread::sleep(std::time::Duration::from_millis(2));
    let b = RequestId::new();
    assert!(
        b.as_str() > a.as_str(),
        "UUID v7 IDs must be lexicographically monotonic: a={a}, b={b}"
    );
}

#[test]
fn request_id_serialize_transparent() {
    let id = RequestId("test-id-value".to_string());
    let serialized = serde_json::to_string(&id).unwrap();
    // Transparent = bare string, not {"0":"..."}
    assert_eq!(serialized, r#""test-id-value""#);
}

#[test]
fn request_id_display() {
    let id = RequestId("hello-world".to_string());
    assert_eq!(id.to_string(), "hello-world");
}

// === Audit JSONL schema v2 ===============================================

#[test]
fn audit_log_includes_request_id_when_provided() {
    let path = tmp_path("with-rid");
    let log = AuditLog::new(Some(path.clone()));
    let rid = "01900000-0000-7000-0000-000000000001";

    log.record("create_page", Some("pg-1"), Ok(()), Some(rid));

    let contents = fs::read_to_string(&path).unwrap();
    let entry: serde_json::Value = serde_json::from_str(contents.trim()).unwrap();
    assert_eq!(
        entry["request_id"].as_str(),
        Some(rid),
        "request_id must be present when provided"
    );
    fs::remove_file(&path).ok();
}

#[test]
fn audit_log_omits_request_id_when_absent() {
    let path = tmp_path("no-rid");
    let log = AuditLog::new(Some(path.clone()));

    log.record("update_page", Some("pg-2"), Ok(()), None);

    let contents = fs::read_to_string(&path).unwrap();
    let entry: serde_json::Value = serde_json::from_str(contents.trim()).unwrap();
    assert!(
        entry.get("request_id").is_none(),
        "request_id must be absent (skip_serializing_if) when None: {entry}"
    );
    fs::remove_file(&path).ok();
}

#[test]
fn audit_v0_3_consumer_can_read_v0_4_entry() {
    // A v0.3-shaped reader (no request_id field) must successfully parse
    // a v0.4 entry that carries a request_id — additive = non-breaking.
    #[derive(serde::Deserialize)]
    struct V3Entry {
        ts: u64,
        privilege: String,
        tool: String,
        result: String,
    }

    let path = tmp_path("compat");
    let log = AuditLog::new(Some(path.clone()));
    log.record("delete_block", Some("blk"), Ok(()), Some("rid-123"));

    let contents = fs::read_to_string(&path).unwrap();
    let v3: V3Entry = serde_json::from_str(contents.trim())
        .expect("v0.3 consumer must parse v0.4 entry without error");
    assert_eq!(v3.tool, "delete_block");
    assert_eq!(v3.privilege, "write");
    assert_eq!(v3.result, "ok");
    assert!(v3.ts > 0);
    fs::remove_file(&path).ok();
}

// === CostEstimate =========================================================

#[test]
fn cost_estimate_single() {
    let e = CostEstimate::single("POST /v1/databases");
    assert_eq!(e.api_calls, 1);
    assert!((e.min_seconds - 1.0 / 3.0).abs() < 1e-9, "min_seconds={}", e.min_seconds);
    assert_eq!(e.endpoints, vec!["POST /v1/databases"]);
}

#[test]
fn cost_estimate_paginated() {
    let e = CostEstimate::paginated("GET /v1/users", 5);
    assert_eq!(e.api_calls, 5);
    assert!((e.min_seconds - 5.0 / 3.0).abs() < 1e-9, "min_seconds={}", e.min_seconds);
    assert_eq!(e.endpoints, vec!["GET /v1/users"]);
}

#[test]
fn cost_estimate_multi() {
    let endpoints = vec![
        "POST /v1/databases".to_string(),
        "PATCH /v1/data_sources/x".to_string(),
    ];
    let e = CostEstimate::multi(endpoints.clone());
    assert_eq!(e.api_calls, 2);
    assert!((e.min_seconds - 2.0 / 3.0).abs() < 1e-9);
    assert_eq!(e.endpoints, endpoints);
}

#[test]
fn cost_estimate_serializes_to_json() {
    let e = CostEstimate::single("POST /v1/pages");
    let v: serde_json::Value = serde_json::to_value(&e).unwrap();
    assert_eq!(v["api_calls"].as_u64(), Some(1));
    assert!(v["min_seconds"].as_f64().is_some());
    assert!(v["endpoints"].as_array().is_some());
}
