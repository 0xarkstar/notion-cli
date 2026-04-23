//! Tests for `--json <body>` universal flag (Justin Poehnelt agent-first CLI
//! principle #1) — parser helpers + CLI integration via `assert_cmd`.

use assert_cmd::Command;

const VALID_ID: &str = "abcdef0123456789abcdef0123456789";
const DB_ID: &str = "fedcba9876543210fedcba9876543210";

fn cli() -> Command {
    let mut cmd = Command::cargo_bin("notion-cli").expect("binary built");
    cmd.env_remove("NOTION_TOKEN");
    cmd
}

fn write_temp_json(name: &str, body: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("notion-cli-json-body-test-{name}.json"));
    std::fs::write(&path, body).expect("write temp json");
    path
}

// === Unit-level parse_json_body tests =====================================

#[test]
fn parse_json_body_literal() {
    use notion_cli::cli::json_body::parse_json_body;
    let v = parse_json_body(r#"{"key":"value"}"#).expect("parses");
    assert_eq!(v["key"], serde_json::json!("value"));
}

#[test]
fn parse_json_body_file() {
    use notion_cli::cli::json_body::parse_json_body;
    let path = write_temp_json("parse_file", r#"{"from_file": true}"#);
    let raw = format!("@{}", path.display());
    let v = parse_json_body(&raw).expect("parses file");
    assert_eq!(v["from_file"], serde_json::json!(true));
    let _ = std::fs::remove_file(path);
}

#[test]
fn parse_json_body_malformed() {
    use notion_cli::cli::json_body::parse_json_body;
    let err = parse_json_body("not json").unwrap_err();
    assert!(err.to_string().contains("not valid JSON"));
}

#[test]
fn parse_json_body_missing_file() {
    use notion_cli::cli::json_body::parse_json_body;
    let err = parse_json_body("@/nonexistent/path/does_not_exist.json").unwrap_err();
    assert!(err.to_string().contains("@/nonexistent"));
}

// === Unit-level reject_json_with_bespoke tests ============================

#[test]
fn reject_json_with_bespoke_no_json() {
    use notion_cli::cli::json_body::reject_json_with_bespoke;
    // has_json = false → always Ok regardless of bespoke flags
    assert!(reject_json_with_bespoke(false, &[("--title", true), ("--icon", true)]).is_ok());
}

#[test]
fn reject_json_with_bespoke_json_no_bespoke() {
    use notion_cli::cli::json_body::reject_json_with_bespoke;
    // has_json = true, all bespoke_flags_present = false → Ok
    assert!(reject_json_with_bespoke(true, &[("--title", false), ("--icon", false)]).is_ok());
}

#[test]
fn reject_json_with_bespoke_mixed() {
    use notion_cli::cli::json_body::reject_json_with_bespoke;
    let err = reject_json_with_bespoke(true, &[("--title", true), ("--icon", false)])
        .unwrap_err();
    assert!(err.to_string().contains("--title"));
    assert!(err.to_string().contains("mutually exclusive"));
}

// === CLI integration tests ================================================

#[test]
fn page_create_with_json_body() {
    let body = serde_json::json!({
        "parent": {"type": "data_source_id", "data_source_id": VALID_ID},
        "properties": {}
    });
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "page",
            "create",
            "--json",
            &body.to_string(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("POST"));
    assert_eq!(parsed.get("path").and_then(|v| v.as_str()), Some("/v1/pages"));
    assert!(parsed.pointer("/body/parent").is_some());
}

#[test]
fn page_create_json_and_title_rejected() {
    // --json + --properties is mutually exclusive
    let body = serde_json::json!({
        "parent": {"type": "data_source_id", "data_source_id": VALID_ID},
        "properties": {}
    });
    let assert = cli()
        .args([
            "--check-request",
            "page",
            "create",
            "--json",
            &body.to_string(),
            "--properties",
            "{}",
        ])
        .assert()
        .failure();
    assert_eq!(assert.get_output().status.code(), Some(64));
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        err.contains("mutually exclusive") || err.contains("--properties"),
        "expected conflict error, got: {err}",
    );
}

#[test]
fn db_update_with_json_body() {
    let body = serde_json::json!({"title": [{"type":"text","text":{"content":"New Title"},"annotations":{"bold":false,"italic":false,"strikethrough":false,"underline":false,"code":false,"color":"default"},"plain_text":"New Title"}]});
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "db",
            "update",
            VALID_ID,
            "--json",
            &body.to_string(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("PATCH"));
    assert!(parsed.pointer("/body/title").is_some());
}

#[test]
fn db_update_json_file_path() {
    let body = serde_json::json!({"in_trash": false});
    let path = write_temp_json("db_update_file", &body.to_string());
    let file_arg = format!("@{}", path.display());
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "db",
            "update",
            VALID_ID,
            "--json",
            &file_arg,
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("PATCH"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn ds_create_with_json_body() {
    let body = serde_json::json!({
        "parent": {"type": "database_id", "database_id": DB_ID},
        "title": [],
        "properties": {"Name": {"title": {}}}
    });
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "ds",
            "create",
            "--json",
            &body.to_string(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(
        parsed.get("path").and_then(|v| v.as_str()),
        Some("/v1/data_sources"),
    );
}

#[test]
fn ds_update_with_json_body_direct() {
    let body = serde_json::json!({
        "properties": {"Priority": {"type": "select", "select": {"options": [{"name": "High"}]}}}
    });
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "ds",
            "update",
            "json",
            VALID_ID,
            &body.to_string(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("PATCH"));
    assert!(parsed.pointer("/body/properties/Priority").is_some());
}

#[test]
fn page_move_with_json_body() {
    let target = "22222222222222222222222222222222";
    let body = serde_json::json!({
        "parent": {"type": "page_id", "page_id": target}
    });
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "page",
            "move",
            VALID_ID,
            "--json",
            &body.to_string(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("POST"));
    let path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
    assert!(path.ends_with("/move"), "expected /move path, got {path}");
    assert_eq!(
        parsed.pointer("/body/parent/type").and_then(|v| v.as_str()),
        Some("page_id"),
    );
}

#[test]
fn page_move_json_and_to_page_rejected() {
    let target = "22222222222222222222222222222222";
    let body = serde_json::json!({"parent": {"type": "page_id", "page_id": target}});
    let assert = cli()
        .args([
            "--check-request",
            "page",
            "move",
            VALID_ID,
            "--json",
            &body.to_string(),
            "--to-page",
            target,
        ])
        .assert()
        .failure();
    assert_eq!(assert.get_output().status.code(), Some(64));
}

#[test]
fn json_body_malformed_exits_2() {
    let assert = cli()
        .args([
            "--check-request",
            "page",
            "create",
            "--json",
            "not json",
        ])
        .assert()
        .failure();
    assert_eq!(assert.get_output().status.code(), Some(2));
}
