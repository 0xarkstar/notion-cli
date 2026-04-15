//! `notion-cli ds *` — data source commands (query, create, retrieve).
//!
//! `ds create` is the-bug endpoint — what the upstream
//! `@notionhq/notion-mcp-server` gets wrong on API 2025-09-03+.

use clap::Subcommand;

use crate::api::data_source::{
    CreateDataSourceParent, CreateDataSourceRequest, QueryDataSourceRequest,
};
use crate::cli::{build_client, Cli, CliError};
use crate::output::emit;
use crate::types::rich_text::{Annotations, RichText, RichTextContent, TextContent};
use crate::validation::{DataSourceId, DatabaseId};

#[derive(Subcommand, Debug)]
pub enum DsCmd {
    /// Retrieve a data source (schema + metadata).
    Get {
        /// Data source ID.
        id: String,
    },
    /// Query pages inside a data source.
    Query {
        /// Data source ID.
        id: String,
        /// Notion filter expression as JSON (see Notion docs).
        #[arg(long)]
        filter: Option<String>,
        /// Sorts as JSON array.
        #[arg(long)]
        sorts: Option<String>,
        /// Pagination cursor.
        #[arg(long)]
        start_cursor: Option<String>,
        /// Results per page (1-100).
        #[arg(long)]
        page_size: Option<u8>,
    },
    /// Create a new data source inside an existing database.
    Create {
        /// Parent database ID or URL.
        #[arg(long)]
        parent: String,
        /// Plain-text title.
        #[arg(long)]
        title: Option<String>,
        /// Property schema JSON (e.g. `{"Name":{"title":{}}}`).
        #[arg(long)]
        properties: String,
    },
}

pub async fn run(cli: &Cli, cmd: &DsCmd) -> Result<(), CliError> {
    match cmd {
        DsCmd::Get { id } => {
            let ds_id = DataSourceId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("data source id: {e}")))?;
            if cli.check_request {
                emit(&cli.output_options(), &serde_json::json!({
                    "method": "GET",
                    "path": format!("/v1/data_sources/{ds_id}"),
                }))?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let ds = client.retrieve_data_source(&ds_id).await?;
            emit(&cli.output_options(), &ds)?;
            Ok(())
        }
        DsCmd::Query {
            id,
            filter,
            sorts,
            start_cursor,
            page_size,
        } => {
            let ds_id = DataSourceId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("data source id: {e}")))?;
            let filter_val = filter
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .map_err(|e| CliError::Validation(format!("--filter: {e}")))?;
            let sorts_vec = sorts
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .map_err(|e| CliError::Validation(format!("--sorts: {e}")))?
                .unwrap_or_default();
            let req = QueryDataSourceRequest {
                filter: filter_val,
                sorts: sorts_vec,
                start_cursor: start_cursor.clone(),
                page_size: *page_size,
            };
            if cli.check_request {
                emit(&cli.output_options(), &serde_json::json!({
                    "method": "POST",
                    "path": format!("/v1/data_sources/{ds_id}/query"),
                    "body": req,
                }))?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let resp = client.query_data_source(&ds_id, &req).await?;
            emit(&cli.output_options(), &resp)?;
            Ok(())
        }
        DsCmd::Create { parent, title, properties } => {
            let db_id = DatabaseId::from_url_or_id(parent)
                .map_err(|e| CliError::Validation(format!("--parent: {e}")))?;
            let props: serde_json::Value = serde_json::from_str(properties)
                .map_err(|e| CliError::Validation(format!("--properties: {e}")))?;
            let title_vec = title
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(plain_title)
                .unwrap_or_default();
            let req = CreateDataSourceRequest {
                parent: CreateDataSourceParent::database(db_id),
                title: title_vec,
                properties: props,
            };
            if cli.check_request {
                emit(&cli.output_options(), &serde_json::json!({
                    "method": "POST",
                    "path": "/v1/data_sources",
                    "body": req,
                }))?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let ds = client.create_data_source(&req).await?;
            emit(&cli.output_options(), &ds)?;
            Ok(())
        }
    }
}

fn plain_title(text: &str) -> Vec<RichText> {
    vec![RichText {
        content: RichTextContent::Text {
            text: TextContent { content: text.to_string(), link: None },
        },
        annotations: Annotations::default(),
        plain_text: text.to_string(),
        href: None,
    }]
}
