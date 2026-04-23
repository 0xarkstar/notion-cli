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

    log.record("create_page", Some("abc123"), Ok(()), None);
    log.record("update_page", Some("xyz789"), Err("validation failed"), None);
    log.record("create_data_source", None, Ok(()), None);

    let contents = fs::read_to_string(&path).expect("audit file exists");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 3, "expected 3 entries, got:\n{contents}");

    let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(first["tool"], "create_page");
    assert_eq!(first["target"], "abc123");
    assert_eq!(first["result"], "ok");
    assert_eq!(
        first["privilege"], "write",
        "v0.3 entries must carry privilege=\"write\" for the write sink",
    );
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

// === Admin sink (D6) =====================================================

#[test]
fn record_admin_writes_to_admin_sink_only() {
    let write_path = tmp_path("split-write");
    let admin_path = tmp_path("split-admin");
    let log = AuditLog::new_with_admin(Some(write_path.clone()), Some(admin_path.clone()));

    log.record("update_page", Some("pg-1"), Ok(()), None);
    log.record_admin("db_create", Some("pg-2"), Ok(()), None);
    log.record_admin("ds_update:remove_property", Some("ds-1"), Err("nope"), None);

    let write_contents = fs::read_to_string(&write_path).expect("write audit file");
    let admin_contents = fs::read_to_string(&admin_path).expect("admin audit file");
    assert_eq!(
        write_contents.lines().count(),
        1,
        "write sink must see exactly the one record() call — got:\n{write_contents}",
    );
    assert_eq!(
        admin_contents.lines().count(),
        2,
        "admin sink must see exactly the two record_admin() calls — got:\n{admin_contents}",
    );

    let admin_first: serde_json::Value =
        serde_json::from_str(admin_contents.lines().next().unwrap()).unwrap();
    assert_eq!(admin_first["privilege"], "admin");
    assert_eq!(admin_first["tool"], "db_create");

    let admin_second: serde_json::Value =
        serde_json::from_str(admin_contents.lines().nth(1).unwrap()).unwrap();
    assert_eq!(admin_second["tool"], "ds_update:remove_property");
    assert_eq!(admin_second["result"], "err");

    fs::remove_file(&write_path).ok();
    fs::remove_file(&admin_path).ok();
}

#[test]
fn record_admin_without_admin_path_is_a_noop() {
    let write_path = tmp_path("admin-noop");
    // write sink set; admin sink unset
    let log = AuditLog::new_with_admin(Some(write_path.clone()), None);
    log.record_admin("db_create", Some("pg"), Ok(()), None);
    // write path must remain empty (record_admin goes to admin sink only)
    let write_contents = fs::read_to_string(&write_path).unwrap_or_default();
    assert!(
        write_contents.is_empty(),
        "record_admin must not fall through to the write sink: {write_contents:?}",
    );
    fs::remove_file(&write_path).ok();
}

#[test]
fn record_falls_through_to_write_sink_even_with_admin_path_set() {
    // Sanity: record() must still go to the write sink, not admin.
    let write_path = tmp_path("write-isolation-w");
    let admin_path = tmp_path("write-isolation-a");
    let log = AuditLog::new_with_admin(Some(write_path.clone()), Some(admin_path.clone()));
    log.record("update_page", Some("pg"), Ok(()), None);
    let admin_contents = fs::read_to_string(&admin_path).unwrap_or_default();
    assert!(
        admin_contents.is_empty(),
        "record() must not leak into the admin sink: {admin_contents:?}",
    );
    let write_contents = fs::read_to_string(&write_path).expect("write sink");
    assert_eq!(write_contents.lines().count(), 1);
    fs::remove_file(&write_path).ok();
    fs::remove_file(&admin_path).ok();
}

#[test]
fn record_with_no_path_is_a_noop() {
    // AuditLog::default() has no path set — must not panic or error.
    let log = AuditLog::default();
    log.record("create_page", Some("abc"), Ok(()), None);
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
                log.record("create_page", Some(&format!("id-{i}")), Ok(()), None);
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
