//! `notion-cli db *` — database container commands.

use std::collections::HashMap;
use std::path::PathBuf;

use clap::Subcommand;

use crate::api::database::{
    CreateDatabaseParent, CreateDatabaseRequest, InitialDataSource,
};
use crate::cli::{build_client, Cli, CliError};
use crate::output::emit;
use crate::types::icon::{Cover, Icon};
use crate::types::property_schema::PropertySchema;
use crate::types::rich_text::RichText;
use crate::validation::{DatabaseId, PageId};

#[derive(Subcommand, Debug)]
pub enum DbCmd {
    /// Retrieve a database container.
    Get {
        /// Database ID or URL.
        id: String,
    },
    /// Create a new database under a parent page.
    ///
    /// Reads the `properties` schema from a JSON file — the expected
    /// shape is `HashMap<String, PropertySchema>`, e.g.:
    ///
    /// ```json
    /// {
    ///   "Name":     {"type": "title", "title": {}},
    ///   "Priority": {"type": "select", "select": {"options": [{"name":"High"}]}},
    ///   "Tags":     {"type": "multi_select", "multi_select": {"options": []}}
    /// }
    /// ```
    ///
    /// Use `notion-cli schema property-schema --pretty` for the full
    /// field reference.
    Create {
        /// Parent page ID or URL.
        #[arg(long)]
        parent_page: String,
        /// Database title (plain text).
        #[arg(long)]
        title: String,
        /// Icon: emoji literal (e.g. `🚀`) or `http(s)://` URL.
        #[arg(long)]
        icon: Option<String>,
        /// Cover image URL.
        #[arg(long)]
        cover: Option<String>,
        /// Mark as inline (rendered inside the parent page instead
        /// of as a child page).
        #[arg(long)]
        inline: bool,
        /// Path to a JSON file containing the initial properties
        /// schema map.
        #[arg(long)]
        schema: PathBuf,
    },
}

pub async fn run(cli: &Cli, cmd: &DbCmd) -> Result<(), CliError> {
    match cmd {
        DbCmd::Get { id } => {
            let db_id = DatabaseId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("database id: {e}")))?;
            if cli.check_request {
                emit(
                    &cli.output_options(),
                    &serde_json::json!({
                        "method": "GET",
                        "path": format!("/v1/databases/{db_id}"),
                    }),
                )?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let db = client.retrieve_database(&db_id).await?;
            emit(&cli.output_options(), &db)?;
            Ok(())
        }
        DbCmd::Create {
            parent_page,
            title,
            icon,
            cover,
            inline,
            schema,
        } => {
            let parent_id = PageId::from_url_or_id(parent_page)
                .map_err(|e| CliError::Validation(format!("--parent-page: {e}")))?;
            let schema_text = std::fs::read_to_string(schema).map_err(|e| {
                CliError::Validation(format!("--schema {}: {e}", schema.display()))
            })?;
            let properties: HashMap<String, PropertySchema> =
                serde_json::from_str(&schema_text).map_err(|e| {
                    CliError::Validation(format!("--schema JSON: {e}"))
                })?;
            let req = CreateDatabaseRequest {
                parent: CreateDatabaseParent::page(parent_id),
                title: RichText::plain(title),
                initial_data_source: InitialDataSource { properties },
                icon: icon.as_deref().map(Icon::parse_cli),
                cover: cover.as_deref().map(Cover::external),
                is_inline: if *inline { Some(true) } else { None },
            };
            req.validate_local()
                .map_err(CliError::Validation)?;
            if cli.check_request {
                emit(
                    &cli.output_options(),
                    &serde_json::json!({
                        "method": "POST",
                        "path": "/v1/databases",
                        "body": req,
                    }),
                )?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let db = client.create_database(&req).await?;
            emit(&cli.output_options(), &db)?;
            Ok(())
        }
    }
}
