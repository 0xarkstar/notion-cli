//! Audit log unit test — validates the write-operation record path
//! used by the MCP full-mode server.

use std::fs;
use std::path::PathBuf;

use notion_cli::mcp::audit::AuditLog;

fn tmp_path(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "notion-cli-audit-{}-{name}.jsonl",
        std::process::id()
    ));
    let _ = fs::remove_file(&p);
    p
}

#[test]
fn record_appends_jsonl_entries() {
    let path = tmp_path("append");
    let log = AuditLog::new(Some(path.clone()));

    log.record("create_page", Some("abc123"), Ok(()));
    log.record("update_page", Some("xyz789"), Err("validation failed"));
    log.record("create_data_source", None, Ok(()));

    let contents = fs::read_to_string(&path).expect("audit file exists");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 3, "expected 3 entries, got:\n{contents}");

    let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(first["tool"], "create_page");
    assert_eq!(first["target"], "abc123");
    assert_eq!(first["result"], "ok");
    assert!(first["error"].is_null());
    assert!(first["ts"].as_u64().unwrap() > 0);

    let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(second["tool"], "update_page");
    assert_eq!(second["result"], "err");
    assert_eq!(second["error"], "validation failed");

    let third: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(third["tool"], "create_data_source");
    assert!(third["target"].is_null());

    fs::remove_file(&path).ok();
}

#[test]
fn record_with_no_path_is_a_noop() {
    // AuditLog::default() has no path set — must not panic or error.
    let log = AuditLog::default();
    log.record("create_page", Some("abc"), Ok(()));
    // nothing to assert beyond "did not panic"
}

#[test]
fn concurrent_records_serialise_cleanly() {
    use std::sync::Arc;
    use std::thread;

    let path = tmp_path("concurrent");
    let log = Arc::new(AuditLog::new(Some(path.clone())));

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let log = log.clone();
            thread::spawn(move || {
                log.record("create_page", Some(&format!("id-{i}")), Ok(()));
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }

    let contents = fs::read_to_string(&path).expect("audit file");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 10);
    for line in lines {
        let _: serde_json::Value = serde_json::from_str(line).expect(
            "every line must be valid JSON — Mutex prevents interleaved writes",
        );
    }

    fs::remove_file(&path).ok();
}
