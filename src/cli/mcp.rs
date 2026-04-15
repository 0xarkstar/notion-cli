//! `notion-cli mcp` — MCP stdio server. Wired in Phase 4.

use crate::cli::CliError;

pub fn run() -> Result<(), CliError> {
    Err(CliError::Usage(
        "MCP server not implemented yet (Phase 4). Current scope is CLI only.".into(),
    ))
}
