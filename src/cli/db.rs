//! `notion-cli db *` — database container commands.

use clap::Subcommand;

use crate::cli::{build_client, Cli, CliError};
use crate::output::emit;
use crate::validation::DatabaseId;

#[derive(Subcommand, Debug)]
pub enum DbCmd {
    /// Retrieve a database container.
    Get {
        /// Database ID or URL.
        id: String,
    },
}

pub async fn run(cli: &Cli, cmd: &DbCmd) -> Result<(), CliError> {
    match cmd {
        DbCmd::Get { id } => {
            let db_id = DatabaseId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("database id: {e}")))?;
            if cli.check_request {
                emit(&cli.output_options(), &serde_json::json!({
                    "method": "GET",
                    "path": format!("/v1/databases/{db_id}"),
                }))?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let db = client.retrieve_database(&db_id).await?;
            emit(&cli.output_options(), &db)?;
            Ok(())
        }
    }
}
