//! CLI integration tests — spawn the compiled `notion-cli` binary
//! via `assert_cmd` and assert on exit code + stdout/stderr shape.
//!
//! These tests exercise:
//! - `--help`, `--version`
//! - `schema <type>` — introspection subcommand
//! - `--check-request` — builds the request body and prints it
//! - Structured exit codes (2/10/64) on validation/config/usage errors
//! - Output envelope (`--raw` vs default) and `--pretty`
//! - No `NOTION_TOKEN` needed when using `--check-request`

use assert_cmd::Command;

const VALID_ID: &str = "abcdef0123456789abcdef0123456789";

fn cli() -> Command {
    let mut cmd = Command::cargo_bin("notion-cli").expect("binary built");
    // Isolate from ambient env — tests must not depend on a real token.
    cmd.env_remove("NOTION_TOKEN");
    cmd
}

// === Help / version =======================================================

#[test]
fn help_exits_0_and_lists_subcommands() {
    let assert = cli().arg("--help").assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    for sub in ["db", "ds", "page", "search", "schema", "mcp"] {
        assert!(out.contains(sub), "help output missing `{sub}`:\n{out}");
    }
}

#[test]
fn version_prints_crate_version() {
    let assert = cli().arg("--version").assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(out.contains(env!("CARGO_PKG_VERSION")), "got: {out}");
}

// === schema subcommand ====================================================

#[test]
fn schema_property_value_emits_valid_json_with_22_variants() {
    let assert = cli().args(["schema", "property-value"]).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    let one_of = parsed
        .get("oneOf")
        .and_then(|v| v.as_array())
        .expect("oneOf present");
    assert_eq!(one_of.len(), 22, "expected 22 PropertyValue variants");
}

#[test]
fn schema_property_emits_valid_json() {
    let assert = cli().args(["schema", "property"]).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(serde_json::from_str::<serde_json::Value>(&out).is_ok());
}

#[test]
fn schema_pretty_produces_indented_output() {
    let assert = cli()
        .args(["--pretty", "schema", "property-value"])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    // Pretty output has multiple lines and leading whitespace
    assert!(out.matches('\n').count() > 10, "pretty output should span many lines");
    assert!(out.contains("  "), "pretty output should be indented");
}

// === --check-request does not need a token ===============================

#[test]
fn check_request_works_without_token() {
    let assert = cli()
        .args(["--check-request", "db", "get", VALID_ID])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    let content = parsed.get("content").unwrap_or(&parsed);
    assert_eq!(content.get("method").and_then(|v| v.as_str()), Some("GET"));
    assert!(content
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .starts_with("/v1/databases/"));
}

#[test]
fn check_request_page_create_prints_body() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "page",
            "create",
            "--parent-data-source",
            VALID_ID,
            "--properties",
            r#"{"Done":{"type":"checkbox","checkbox":true}}"#,
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("POST"));
    assert_eq!(parsed.get("path").and_then(|v| v.as_str()), Some("/v1/pages"));
    let body = parsed.get("body").expect("body present");
    assert!(body.get("properties").is_some());
    assert!(body.get("parent").is_some());
}

#[test]
fn check_request_ds_create_hits_the_bug_endpoint() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "ds",
            "create",
            "--parent",
            VALID_ID,
            "--title",
            "test-ds",
            "--properties",
            r#"{"Name":{"title":{}}}"#,
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("POST"));
    assert_eq!(
        parsed.get("path").and_then(|v| v.as_str()),
        Some("/v1/data_sources"),
        "the-bug path must be /v1/data_sources, not /v1/databases/{{id}}/...",
    );
}

// === db create (v0.3 admin) ==============================================

fn write_temp_schema(name: &str, body: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("notion-cli-test-schema-{name}.json"));
    std::fs::write(&path, body).expect("write temp schema");
    path
}

#[test]
fn check_request_db_create_uses_page_parent_and_databases_path() {
    let schema_path = write_temp_schema(
        "db_create_ok",
        r#"{"Name":{"type":"title","title":{}}}"#,
    );
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "db",
            "create",
            "--parent-page",
            VALID_ID,
            "--title",
            "TestDB",
            "--schema",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("POST"));
    assert_eq!(parsed.get("path").and_then(|v| v.as_str()), Some("/v1/databases"));
    let parent_type = parsed
        .pointer("/body/parent/type")
        .and_then(|v| v.as_str());
    assert_eq!(parent_type, Some("page_id"), "parent must be page_id");
    let _ = std::fs::remove_file(schema_path);
}

#[test]
fn db_create_without_title_property_exits_with_validation_code_2() {
    let schema_path = write_temp_schema(
        "db_create_no_title",
        r#"{"Done":{"type":"checkbox","checkbox":{}}}"#,
    );
    let assert = cli()
        .args([
            "--check-request",
            "db",
            "create",
            "--parent-page",
            VALID_ID,
            "--title",
            "BadDB",
            "--schema",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .failure();
    assert_eq!(assert.get_output().status.code(), Some(2));
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        err.to_lowercase().contains("title"),
        "expected title-prop validation error in stderr: {err}",
    );
    let _ = std::fs::remove_file(schema_path);
}

#[test]
fn db_create_with_emoji_icon_parses_to_emoji_shape() {
    let schema_path = write_temp_schema(
        "db_create_emoji",
        r#"{"Name":{"type":"title","title":{}}}"#,
    );
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "db",
            "create",
            "--parent-page",
            VALID_ID,
            "--title",
            "IconDB",
            "--icon",
            "🚀",
            "--schema",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    let icon_type = parsed.pointer("/body/icon/type").and_then(|v| v.as_str());
    assert_eq!(icon_type, Some("emoji"));
    let emoji = parsed.pointer("/body/icon/emoji").and_then(|v| v.as_str());
    assert_eq!(emoji, Some("🚀"));
    let _ = std::fs::remove_file(schema_path);
}

// === ds update (v0.3 admin) ==============================================

#[test]
fn ds_update_add_property_check_request_patches_data_source_path() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "ds",
            "update",
            "add-property",
            VALID_ID,
            "--name",
            "Priority",
            "--schema",
            r#"{"type":"select","select":{"options":[{"name":"High"}]}}"#,
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("PATCH"));
    let path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        path.starts_with("/v1/data_sources/") && path.len() > "/v1/data_sources/".len(),
        "expected /v1/data_sources/{{id}} path, got {path}"
    );
    let prop_type = parsed
        .pointer("/body/properties/Priority/type")
        .and_then(|v| v.as_str());
    assert_eq!(prop_type, Some("select"));
}

#[test]
fn ds_update_remove_property_without_yes_exits_64() {
    // Remove-property is destructive; --yes is required at the CLI
    // surface when not running in a TTY (D1). assert_cmd never has
    // a TTY, so absence of --yes must trip the usage guard.
    let assert = cli()
        .args([
            "--check-request",
            "ds",
            "update",
            "remove-property",
            VALID_ID,
            "--name",
            "Doomed",
        ])
        .assert()
        .failure();
    assert_eq!(assert.get_output().status.code(), Some(64));
    let err = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        err.to_lowercase().contains("destructive") || err.to_lowercase().contains("--yes"),
        "expected destructive/--yes hint in stderr: {err}"
    );
}

#[test]
fn ds_update_remove_property_with_yes_sends_null() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "ds",
            "update",
            "remove-property",
            VALID_ID,
            "--name",
            "Doomed",
            "--yes",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    let doomed = parsed.pointer("/body/properties/Doomed");
    assert_eq!(
        doomed,
        Some(&serde_json::Value::Null),
        "remove-property must emit null for the doomed key"
    );
}

#[test]
fn ds_update_rename_property_sends_name_directive() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "ds",
            "update",
            "rename-property",
            VALID_ID,
            "--from",
            "Old",
            "--to",
            "New",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    let new_name = parsed
        .pointer("/body/properties/Old/name")
        .and_then(|v| v.as_str());
    assert_eq!(new_name, Some("New"));
}

#[test]
fn ds_update_add_option_emits_merge_delta() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "ds",
            "update",
            "add-option",
            VALID_ID,
            "--property",
            "Priority",
            "--name",
            "Urgent",
            "--color",
            "red",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    let first_opt = parsed
        .pointer("/body/properties/Priority/select/options/0/name")
        .and_then(|v| v.as_str());
    assert_eq!(first_opt, Some("Urgent"));
    let first_color = parsed
        .pointer("/body/properties/Priority/select/options/0/color")
        .and_then(|v| v.as_str());
    assert_eq!(first_color, Some("red"));
}

#[test]
fn db_create_with_url_icon_parses_to_external_shape() {
    let schema_path = write_temp_schema(
        "db_create_url_icon",
        r#"{"Name":{"type":"title","title":{}}}"#,
    );
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "db",
            "create",
            "--parent-page",
            VALID_ID,
            "--title",
            "UrlIconDB",
            "--icon",
            "https://example.com/icon.png",
            "--schema",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    let icon_type = parsed.pointer("/body/icon/type").and_then(|v| v.as_str());
    assert_eq!(icon_type, Some("external"));
    let _ = std::fs::remove_file(schema_path);
}

// === Output envelope ======================================================

#[test]
fn default_output_wraps_in_untrusted_envelope() {
    let assert = cli()
        .args(["--check-request", "db", "get", VALID_ID])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(parsed.get("source").and_then(|v| v.as_str()), Some("notion"));
    assert_eq!(parsed.get("trust").and_then(|v| v.as_str()), Some("untrusted"));
    assert!(parsed.get("api_version").is_some());
    assert!(parsed.get("content").is_some());
}

#[test]
fn raw_skips_envelope() {
    let assert = cli()
        .args(["--check-request", "--raw", "db", "get", VALID_ID])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert!(parsed.get("source").is_none(), "raw output must not include envelope");
    assert!(parsed.get("content").is_none());
    assert!(parsed.get("method").is_some(), "content inlined at top level");
}

// === Structured exit codes ================================================

#[test]
fn bad_id_exits_with_validation_code_2() {
    let assert = cli().args(["db", "get", "not-an-id"]).assert().failure();
    assert_eq!(assert.get_output().status.code(), Some(2));
}

#[test]
fn missing_token_exits_with_config_code_10() {
    // No --check-request, no NOTION_TOKEN env → must fail at config step
    // before any network call.
    let assert = cli().args(["db", "get", VALID_ID]).assert().failure();
    assert_eq!(assert.get_output().status.code(), Some(10));
}

#[test]
fn mcp_without_token_exits_with_config_code_10() {
    // MCP server requires a token to build the Notion client. Without
    // one, fail closed at config stage before entering the stdio loop.
    let assert = cli().args(["mcp"]).assert().failure();
    assert_eq!(assert.get_output().status.code(), Some(10));
}

#[test]
fn mcp_help_lists_allow_write_flag() {
    let assert = cli().args(["mcp", "--help"]).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(out.contains("--allow-write"), "help missing --allow-write:\n{out}");
    assert!(out.contains("--audit-log"), "help missing --audit-log:\n{out}");
}

#[test]
fn page_create_without_parent_exits_with_usage_code_64() {
    let assert = cli()
        .args([
            "--check-request",
            "page",
            "create",
            "--properties",
            "{}",
        ])
        .assert()
        .failure();
    assert_eq!(assert.get_output().status.code(), Some(64));
}

#[test]
fn invalid_properties_json_exits_validation_code_2() {
    let assert = cli()
        .args([
            "--check-request",
            "page",
            "create",
            "--parent-data-source",
            VALID_ID,
            "--properties",
            "not valid json",
        ])
        .assert()
        .failure();
    assert_eq!(assert.get_output().status.code(), Some(2));
}

// === Block commands (--check-request) ====================================

const BLOCK_ID: &str = "cccccccccccccccccccccccccccccccc";

#[test]
fn check_request_block_get() {
    let assert = cli()
        .args(["--check-request", "--raw", "block", "get", BLOCK_ID])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("GET"));
    assert!(parsed
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .starts_with("/v1/blocks/"));
}

#[test]
fn check_request_block_list_with_cursor() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "block",
            "list",
            BLOCK_ID,
            "--start-cursor",
            "abc",
            "--page-size",
            "10",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    let path_s = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
    assert!(path_s.contains("/children"));
    assert!(path_s.contains("start_cursor=abc"));
    assert!(path_s.contains("page_size=10"));
}

#[test]
fn check_request_block_append_typed() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "block",
            "append",
            BLOCK_ID,
            "--children",
            r#"[{"type":"paragraph","paragraph":{"rich_text":[],"color":"default"}}]"#,
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("PATCH"));
    assert!(parsed.pointer("/body/children").is_some());
}

#[test]
fn check_request_block_append_rejects_bad_json() {
    let assert = cli()
        .args([
            "--check-request",
            "block",
            "append",
            BLOCK_ID,
            "--children",
            "not json",
        ])
        .assert()
        .failure();
    assert_eq!(assert.get_output().status.code(), Some(2));
}

#[test]
fn check_request_block_update_archive() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "block",
            "update",
            BLOCK_ID,
            "--archived",
            "true",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("PATCH"));
    assert_eq!(
        parsed.pointer("/body/archived").and_then(serde_json::Value::as_bool),
        Some(true),
    );
}

#[test]
fn check_request_block_delete() {
    let assert = cli()
        .args(["--check-request", "--raw", "block", "delete", BLOCK_ID])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("DELETE"));
}

#[test]
fn check_request_page_create_with_children() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "page",
            "create",
            "--parent-data-source",
            VALID_ID,
            "--properties",
            "{}",
            "--children",
            r#"[{"type":"paragraph","paragraph":{"rich_text":[],"color":"default"}}]"#,
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    let children = parsed.pointer("/body/children").and_then(|v| v.as_array());
    assert_eq!(children.map(std::vec::Vec::len), Some(1));
}

#[test]
fn schema_block_type() {
    let assert = cli().args(["schema", "property-value"]).assert().success();
    let _out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    // Also add a quick block schema check
    let assert2 = cli().args(["block", "--help"]).assert().success();
    let out2 = String::from_utf8_lossy(&assert2.get_output().stdout).to_string();
    for sub in ["get", "list", "append", "update", "delete"] {
        assert!(out2.contains(sub), "block help missing subcommand `{sub}`:\n{out2}");
    }
}

// === Token can be passed via --token flag =================================

// === --check-request covers each verb's parsing path ====================

const DS_ID: &str = "fedcba9876543210fedcba9876543210";
const PAGE_ID: &str = "11111111111111111111111111111111";

#[test]
fn check_request_ds_get() {
    let assert = cli()
        .args(["--check-request", "--raw", "ds", "get", VALID_ID])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("GET"));
    assert!(parsed
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .starts_with("/v1/data_sources/"));
}

#[test]
fn check_request_ds_query_with_filter() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "ds",
            "query",
            DS_ID,
            "--filter",
            r#"{"property":"Done","checkbox":{"equals":true}}"#,
            "--page-size",
            "25",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("POST"));
    assert!(parsed
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .contains("/query"));
    assert!(parsed.pointer("/body/filter").is_some());
    assert_eq!(
        parsed.pointer("/body/page_size").and_then(serde_json::Value::as_u64),
        Some(25),
    );
}

#[test]
fn check_request_ds_query_rejects_bad_filter_json() {
    let assert = cli()
        .args([
            "--check-request",
            "ds",
            "query",
            DS_ID,
            "--filter",
            "not json",
        ])
        .assert()
        .failure();
    assert_eq!(assert.get_output().status.code(), Some(2));
}

#[test]
fn check_request_page_get() {
    let assert = cli()
        .args(["--check-request", "--raw", "page", "get", PAGE_ID])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("GET"));
}

#[test]
fn check_request_page_update() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "page",
            "update",
            PAGE_ID,
            "--archived",
            "true",
            "--properties",
            r#"{"Done":{"type":"checkbox","checkbox":false}}"#,
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("PATCH"));
    assert_eq!(
        parsed.pointer("/body/archived").and_then(serde_json::Value::as_bool),
        Some(true),
    );
}

#[test]
fn check_request_page_archive_sets_in_trash() {
    let assert = cli()
        .args(["--check-request", "--raw", "page", "archive", PAGE_ID])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed.get("method").and_then(|v| v.as_str()), Some("PATCH"));
    assert_eq!(
        parsed.pointer("/body/in_trash").and_then(serde_json::Value::as_bool),
        Some(true),
    );
}

#[test]
fn check_request_page_create_with_page_parent() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "page",
            "create",
            "--parent-page",
            PAGE_ID,
            "--properties",
            "{}",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        parsed.pointer("/body/parent/type").and_then(|v| v.as_str()),
        Some("page_id"),
    );
}

#[test]
fn check_request_search_with_query() {
    let assert = cli()
        .args([
            "--check-request",
            "--raw",
            "search",
            "hello",
            "--filter",
            r#"{"property":"object","value":"page"}"#,
            "--page-size",
            "10",
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        parsed.get("path").and_then(|v| v.as_str()),
        Some("/v1/search"),
    );
    assert_eq!(
        parsed.pointer("/body/query").and_then(|v| v.as_str()),
        Some("hello"),
    );
}

#[test]
fn check_request_search_empty_query() {
    let assert = cli()
        .args(["--check-request", "--raw", "search"])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        parsed.get("path").and_then(|v| v.as_str()),
        Some("/v1/search"),
    );
}

#[test]
fn check_request_search_rejects_bad_filter() {
    let assert = cli()
        .args(["--check-request", "search", "--filter", "not json"])
        .assert()
        .failure();
    assert_eq!(assert.get_output().status.code(), Some(2));
}

#[test]
fn check_request_ds_create_rejects_bad_properties() {
    let assert = cli()
        .args([
            "--check-request",
            "ds",
            "create",
            "--parent",
            VALID_ID,
            "--properties",
            "not json",
        ])
        .assert()
        .failure();
    assert_eq!(assert.get_output().status.code(), Some(2));
}

#[test]
fn check_request_ds_create_rejects_bad_parent() {
    let assert = cli()
        .args([
            "--check-request",
            "ds",
            "create",
            "--parent",
            "not-a-db",
            "--properties",
            "{}",
        ])
        .assert()
        .failure();
    assert_eq!(assert.get_output().status.code(), Some(2));
}

#[test]
fn check_request_page_create_rejects_bad_parent_data_source() {
    let assert = cli()
        .args([
            "--check-request",
            "page",
            "create",
            "--parent-data-source",
            "not-an-id",
            "--properties",
            "{}",
        ])
        .assert()
        .failure();
    assert_eq!(assert.get_output().status.code(), Some(2));
}

#[test]
fn schema_filter() {
    let assert = cli().args(["schema", "filter"]).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(serde_json::from_str::<serde_json::Value>(&out).is_ok());
}

#[test]
fn schema_sort() {
    let assert = cli().args(["schema", "sort"]).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(serde_json::from_str::<serde_json::Value>(&out).is_ok());
}

#[test]
fn schema_page() {
    let assert = cli().args(["schema", "page"]).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(serde_json::from_str::<serde_json::Value>(&out).is_ok());
}

#[test]
fn schema_database() {
    let assert = cli().args(["schema", "database"]).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(serde_json::from_str::<serde_json::Value>(&out).is_ok());
}

#[test]
fn schema_data_source() {
    let assert = cli().args(["schema", "data-source"]).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(serde_json::from_str::<serde_json::Value>(&out).is_ok());
}

#[test]
fn schema_rich_text() {
    let assert = cli().args(["schema", "rich-text"]).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(serde_json::from_str::<serde_json::Value>(&out).is_ok());
}

#[test]
fn token_via_flag_overrides_env() {
    // We're not making a real call — just asserting the flag parses.
    // If the token flag was required, --check-request would still work.
    let assert = cli()
        .args(["--token", "ntn_flag_value", "--check-request", "db", "get", VALID_ID])
        .assert()
        .success();
    // Token must not leak to stdout.
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(!out.contains("ntn_flag_value"), "token leaked to stdout: {out}");
}
