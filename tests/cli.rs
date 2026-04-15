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

// === Token can be passed via --token flag =================================

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
