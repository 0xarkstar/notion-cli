//! Append-only audit log for MCP write operations.
//!
//! Best-effort: failures are logged to stderr but do not fail the
//! tool call. The log path is configured via `--audit-log` (CLI) or
//! `NOTION_CLI_AUDIT_LOG` env var. When unset, auditing is disabled.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

#[derive(Debug, Serialize)]
struct AuditEntry<'a> {
    ts: u64,
    tool: &'a str,
    target: Option<&'a str>,
    result: &'a str,
    error: Option<&'a str>,
}

#[derive(Default)]
pub struct AuditLog {
    path: Option<PathBuf>,
    // Mutex to serialize appends across concurrent tool calls.
    writer: Mutex<()>,
}

impl AuditLog {
    #[must_use]
    pub fn new(path: Option<PathBuf>) -> Self {
        Self { path, writer: Mutex::new(()) }
    }

    pub fn record(
        &self,
        tool: &str,
        target: Option<&str>,
        result: Result<(), &str>,
    ) {
        let Some(path) = &self.path else { return };
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let (result_str, error) = match result {
            Ok(()) => ("ok", None),
            Err(msg) => ("err", Some(msg)),
        };
        let entry = AuditEntry { ts, tool, target, result: result_str, error };
        let line = match serde_json::to_string(&entry) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("audit: serialise failed: {e}");
                return;
            }
        };
        let _guard = self.writer.lock();
        if let Err(e) = append_line(path, &line) {
            eprintln!("audit: write {} failed: {e}", path.display());
        }
    }
}

fn append_line(path: &std::path::Path, line: &str) -> std::io::Result<()> {
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(f, "{line}")
}
