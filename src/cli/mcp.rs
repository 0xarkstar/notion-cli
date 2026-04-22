//! `notion-cli mcp [--allow-write | --allow-admin] [--audit-log PATH] [--admin-log PATH]`
//! — MCP stdio server.

use std::path::PathBuf;

use clap::Args;

use crate::cli::{build_client, Cli, CliError};

#[derive(Args, Debug)]
pub struct McpArgs {
    /// Expose runtime write tools (`create_page`, `update_page`,
    /// `create_data_source`, `append/update/delete_block`).
    ///
    /// Without this flag the server is read-only. Mutually exclusive
    /// with `--allow-admin` (admin is a superset — use that instead
    /// if you need admin ops).
    #[arg(long, conflicts_with = "allow_admin")]
    pub allow_write: bool,

    /// Expose admin lifecycle tools (`db_create`, `ds_update`,
    /// `ds_add_relation`, `page_move`) on top of the 12 runtime
    /// tools.
    ///
    /// This is **tool-exposure policy**, NOT a security boundary:
    /// an agent with an admin-scoped integration token + code
    /// execution can hit the API directly regardless of MCP
    /// gating. What `--allow-admin` provides is prompt-injection
    /// attenuation (admin tools absent from the agent's planning
    /// surface) and accidental-action prevention (operator can't
    /// fat-finger schema mutation through an agent meant to be
    /// read/write only).
    #[arg(long)]
    pub allow_admin: bool,

    /// Append-only JSONL audit log for write operations.
    ///
    /// Defaults to the `NOTION_CLI_AUDIT_LOG` env var. When unset,
    /// write auditing is disabled.
    #[arg(long, env = "NOTION_CLI_AUDIT_LOG")]
    pub audit_log: Option<PathBuf>,

    /// Append-only JSONL audit log for admin lifecycle operations
    /// (higher-privilege than write).
    ///
    /// Defaults to the `NOTION_CLI_ADMIN_LOG` env var. When unset,
    /// admin auditing is disabled. Separating this from the write
    /// log lets operators grep-split agent activity vs structural
    /// mutation without jq filters.
    #[arg(long, env = "NOTION_CLI_ADMIN_LOG")]
    pub admin_log: Option<PathBuf>,
}

pub async fn run(cli: &Cli, args: &McpArgs) -> Result<(), CliError> {
    let client = build_client(cli)?;
    let result = if args.allow_admin {
        crate::mcp::run_with_admin(client, args.audit_log.clone(), args.admin_log.clone()).await
    } else if args.allow_write {
        crate::mcp::run_with_write(client, args.audit_log.clone()).await
    } else {
        crate::mcp::run_read_only(client).await
    };
    result.map_err(|e| CliError::Config(format!("MCP server: {e}")))
}
