//! CLI surface. Parsed by clap; dispatched by [`run`].
//!
//! # Global flags
//! - `--check-request` — build the request body, print it, do not
//!   contact Notion. Local structural validation only; does not
//!   simulate server-side effects.
//! - `--raw` — skip the untrusted-source envelope in output.
//! - `--pretty` — indented JSON output.
//! - `--token <TOKEN>` — overrides the `NOTION_TOKEN` env var.
//!
//! # Exit codes
//! - 0: success
//! - 2: validation error (client-side or from Notion)
//! - 3: API error (non-validation)
//! - 4: rate-limited (after retries exhausted)
//! - 10: config / auth error
//! - 64: usage error (bad arguments)
//! - 65: JSON parse error
//! - 74: I/O error

pub mod block;
pub mod db;
pub mod ds;
pub mod error;
pub mod mcp;
pub mod page;
pub mod schema;
pub mod search;
pub mod user;

use clap::{Parser, Subcommand};

use crate::api::NotionClient;
use crate::config::NotionToken;
use crate::output::OutputOptions;

pub use error::CliError;

#[derive(Parser, Debug)]
#[command(
    name = "notion-cli",
    version,
    about = "Agent-First Notion CLI and MCP server",
    long_about = None,
)]
pub struct Cli {
    /// Validate the request locally; do not call Notion.
    ///
    /// Checks structural validity only — not permissions,
    /// referential integrity, or server-side business rules.
    #[arg(long, global = true)]
    pub check_request: bool,

    /// Skip wrapping output in the untrusted-source envelope.
    #[arg(long, global = true)]
    pub raw: bool,

    /// Pretty-print JSON output with indentation.
    #[arg(long, global = true)]
    pub pretty: bool,

    /// Notion integration token (defaults to $`NOTION_TOKEN`).
    #[arg(long, env = "NOTION_TOKEN", hide_env_values = true, global = true)]
    pub token: Option<String>,

    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Database container operations.
    #[command(subcommand)]
    Db(db::DbCmd),
    /// Data source operations (schema + queries).
    #[command(subcommand)]
    Ds(ds::DsCmd),
    /// Page operations.
    #[command(subcommand)]
    Page(page::PageCmd),
    /// Block operations — retrieve, list children, append, update, delete.
    #[command(subcommand)]
    Block(block::BlockCmd),
    /// Full-text search across pages / data sources / databases.
    Search(search::SearchArgs),
    /// Print JSON Schema for an internal type.
    Schema(schema::SchemaArgs),
    /// User enumeration (CLI-only — not exposed over MCP in v0.3).
    #[command(subcommand)]
    Users(user::UsersCmd),
    /// Start an MCP stdio server for agent integration (Hermes, Claude).
    Mcp(mcp::McpArgs),
}

impl Cli {
    pub fn output_options(&self) -> OutputOptions {
        OutputOptions { raw: self.raw, pretty: self.pretty }
    }
}

/// Build a client from the resolved token, unless `--check-request`
/// is set (in which case no client is needed).
pub fn build_client(cli: &Cli) -> Result<NotionClient, CliError> {
    let token_str = cli.token.as_deref().ok_or_else(|| {
        CliError::Config(
            "missing NOTION_TOKEN (set env var or pass --token)".to_string(),
        )
    })?;
    NotionClient::new(&NotionToken::new(token_str))
        .map_err(|e| CliError::Config(format!("client init: {e}")))
}

/// Dispatch a parsed [`Cli`] to the right subcommand handler.
///
/// # Errors
/// Returns a [`CliError`] with a meaningful exit code on any failure.
pub async fn run(cli: Cli) -> Result<(), CliError> {
    match &cli.cmd {
        Command::Db(cmd) => db::run(&cli, cmd).await,
        Command::Ds(cmd) => ds::run(&cli, cmd).await,
        Command::Page(cmd) => page::run(&cli, cmd).await,
        Command::Block(cmd) => block::run(&cli, cmd).await,
        Command::Search(args) => search::run(&cli, args).await,
        Command::Schema(args) => schema::run(&cli, args),
        Command::Users(cmd) => user::run(&cli, cmd).await,
        Command::Mcp(args) => mcp::run(&cli, args).await,
    }
}
