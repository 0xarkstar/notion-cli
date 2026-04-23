//! Append-only audit log for MCP write + admin operations.
//!
//! Two sinks (D6):
//! - `NOTION_CLI_AUDIT_LOG` — runtime writes
//!   (`create_page`, `update_page`, `create_data_source`, block mutations).
//! - `NOTION_CLI_ADMIN_LOG` — admin lifecycle ops
//!   (`db_create`, `ds_update:*`, `ds_add_relation`, `page_move`).
//!
//! Operators can `grep`-split by file for "what did agents do?" vs
//! "what did I mutate in the schema?" without jq. Every entry also
//! carries an explicit `privilege` field (`"write"` or `"admin"`)
//! so the two logs stay self-describing if ever merged.
//!
//! Best-effort: failures are logged to stderr but do not fail the
//! tool call. When the relevant path is unset, logging is skipped.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

#[derive(Debug, Serialize)]
struct AuditEntry<'a> {
    ts: u64,
    privilege: &'a str,
    tool: &'a str,
    target: Option<&'a str>,
    result: &'a str,
    error: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<&'a str>,
}

#[derive(Default)]
pub struct AuditLog {
    write_path: Option<PathBuf>,
    admin_path: Option<PathBuf>,
    // Mutex to serialize appends across concurrent tool calls (both sinks).
    writer: Mutex<()>,
}

impl AuditLog {
    /// Construct with write-tier logging only. Admin ops will not be
    /// audited. Compat with v0.2 behaviour — callers upgrade to
    /// [`Self::new_with_admin`] when they need the admin sink.
    #[must_use]
    pub fn new(write_path: Option<PathBuf>) -> Self {
        Self {
            write_path,
            admin_path: None,
            writer: Mutex::new(()),
        }
    }

    /// Construct with both sinks. Either path can be `None` to
    /// disable that tier individually.
    #[must_use]
    pub fn new_with_admin(
        write_path: Option<PathBuf>,
        admin_path: Option<PathBuf>,
    ) -> Self {
        Self {
            write_path,
            admin_path,
            writer: Mutex::new(()),
        }
    }

    /// Record a runtime write operation. Goes to the write sink.
    pub fn record(
        &self,
        tool: &str,
        target: Option<&str>,
        result: Result<(), &str>,
        request_id: Option<&str>,
    ) {
        self.append(self.write_path.as_deref(), "write", tool, target, result, request_id);
    }

    /// Record an admin lifecycle operation. Goes to the admin sink.
    pub fn record_admin(
        &self,
        tool: &str,
        target: Option<&str>,
        result: Result<(), &str>,
        request_id: Option<&str>,
    ) {
        self.append(self.admin_path.as_deref(), "admin", tool, target, result, request_id);
    }

    fn append(
        &self,
        path: Option<&std::path::Path>,
        privilege: &str,
        tool: &str,
        target: Option<&str>,
        result: Result<(), &str>,
        request_id: Option<&str>,
    ) {
        let Some(path) = path else { return };
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let (result_str, error) = match result {
            Ok(()) => ("ok", None),
            Err(msg) => ("err", Some(msg)),
        };
        let entry = AuditEntry {
            ts,
            privilege,
            tool,
            target,
            result: result_str,
            error,
            request_id,
        };
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
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{line}")
}
