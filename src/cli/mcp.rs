//! `notion-cli mcp [--allow-write] [--audit-log PATH]` — MCP stdio server.

use std::path::PathBuf;

use clap::Args;

use crate::cli::{build_client, Cli, CliError};

#[derive(Args, Debug)]
pub struct McpArgs {
    /// Expose write tools (`create_page`, `update_page`, `create_data_source`).
    ///
    /// Without this flag the server is read-only (`get_page`,
    /// `get_data_source`, `query_data_source`, search only). Read-only is
    /// the safe default per the security model — untrusted Notion
    /// content can drive agent behaviour, so destructive ops are
    /// gated at the server surface.
    #[arg(long)]
    pub allow_write: bool,

    /// Append-only JSONL audit log for write operations.
    ///
    /// Defaults to the `NOTION_CLI_AUDIT_LOG` env var. When unset,
    /// audit logging is disabled.
    #[arg(long, env = "NOTION_CLI_AUDIT_LOG")]
    pub audit_log: Option<PathBuf>,
}

pub async fn run(cli: &Cli, args: &McpArgs) -> Result<(), CliError> {
    let client = build_client(cli)?;
    let result = if args.allow_write {
        crate::mcp::run_with_write(client, args.audit_log.clone()).await
    } else {
        crate::mcp::run_read_only(client).await
    };
    result.map_err(|e| CliError::Config(format!("MCP server: {e}")))
}
