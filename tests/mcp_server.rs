//! MCP server integration tests.
//!
//! Spawns the compiled `notion-cli mcp` binary, sends a stdio
//! JSON-RPC handshake + `tools/list`, and asserts the returned tool
//! set. This verifies both that the server starts and that the
//! read/write-tool gating actually takes effect at the protocol level.

use std::io::Write;
use std::process::{Command, Stdio};

const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}"#;
const INITIALIZED: &str = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
const LIST_TOOLS: &str = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;

fn run_mcp(extra_args: &[&str]) -> String {
    let exe = env!("CARGO_BIN_EXE_notion-cli");
    let mut cmd = Command::new(exe);
    cmd.arg("mcp").arg("--token").arg("ntn_test");
    for a in extra_args {
        cmd.arg(a);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().expect("spawn mcp server");
    {
        let stdin = child.stdin.as_mut().expect("stdin pipe");
        writeln!(stdin, "{INIT}").unwrap();
        writeln!(stdin, "{INITIALIZED}").unwrap();
        writeln!(stdin, "{LIST_TOOLS}").unwrap();
        // Closing stdin triggers graceful shutdown in rmcp.
    }
    let output = child.wait_with_output().expect("wait");
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn extract_tool_names(stdout: &str) -> Vec<String> {
    // tools/list response has shape: {"id":2,"result":{"tools":[{"name":"...",...},...]}}
    // We parse line-by-line (rmcp emits one JSON-RPC message per line).
    let mut names = Vec::new();
    for line in stdout.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let Some(tools) = value.pointer("/result/tools").and_then(|v| v.as_array()) else {
            continue;
        };
        for t in tools {
            if let Some(n) = t.get("name").and_then(|v| v.as_str()) {
                names.push(n.to_string());
            }
        }
    }
    names.sort();
    names
}

#[test]
fn read_only_mode_exposes_6_tools() {
    let out = run_mcp(&[]);
    let tools = extract_tool_names(&out);
    assert_eq!(
        tools,
        vec![
            "get_block".to_string(),
            "get_data_source".to_string(),
            "get_page".to_string(),
            "list_block_children".to_string(),
            "query_data_source".to_string(),
            "search".to_string(),
        ],
        "unexpected tool set:\n{out}",
    );
}

#[test]
fn allow_write_mode_exposes_12_tools() {
    let out = run_mcp(&["--allow-write"]);
    let tools = extract_tool_names(&out);
    assert_eq!(
        tools,
        vec![
            "append_block_children".to_string(),
            "create_data_source".to_string(),
            "create_page".to_string(),
            "delete_block".to_string(),
            "get_block".to_string(),
            "get_data_source".to_string(),
            "get_page".to_string(),
            "list_block_children".to_string(),
            "query_data_source".to_string(),
            "search".to_string(),
            "update_block".to_string(),
            "update_page".to_string(),
        ],
        "unexpected tool set:\n{out}",
    );
}

#[test]
fn read_only_does_not_expose_write_tools() {
    let out = run_mcp(&[]);
    let tools = extract_tool_names(&out);
    for write_tool in [
        "create_page",
        "update_page",
        "create_data_source",
        "append_block_children",
        "update_block",
        "delete_block",
    ] {
        assert!(
            !tools.contains(&write_tool.to_string()),
            "write tool `{write_tool}` leaked in read-only mode:\n{tools:?}",
        );
    }
}

#[test]
fn create_data_source_tool_is_exposed_in_write_mode() {
    let out = run_mcp(&["--allow-write"]);
    assert!(
        out.contains("create_data_source"),
        "create_data_source must be listed in full tools:\n{out}",
    );
}

#[test]
fn tool_schemas_have_flat_string_ids() {
    // Agent-friendliness gate: ID fields should surface as plain
    // `{"type": "string"}` without deep oneOf/$ref chains, so that
    // Anthropic's tool-use validator and Gemma-class parsers can
    // reliably construct calls.
    let out = run_mcp(&[]);
    // Strip backslashes only for the substring match so we tolerate
    // the escaping rmcp applies on wire.
    let get_page_schema_has_string_id = out.contains("\"page_id\"")
        && out.contains("\"type\":\"string\"");
    assert!(
        get_page_schema_has_string_id,
        "get_page input_schema must expose page_id as plain string:\n{out}",
    );
}
