//! `notion-cli page *` — page commands.

use std::collections::HashMap;

use clap::Subcommand;

use crate::api::page::{CreatePageRequest, PageParent, UpdatePageRequest};
use crate::cli::{build_client, Cli, CliError};
use crate::output::emit;
use crate::types::property::PropertyValue;
use crate::validation::{DataSourceId, PageId};

#[derive(Subcommand, Debug)]
pub enum PageCmd {
    /// Retrieve a page.
    Get {
        /// Page ID or URL.
        id: String,
    },
    /// Create a page under a data source (or another page).
    Create {
        /// Parent data source ID (mutually exclusive with --parent-page).
        #[arg(long, group = "parent", conflicts_with = "parent_page")]
        parent_data_source: Option<String>,
        /// Parent page ID (mutually exclusive with --parent-data-source).
        #[arg(long, group = "parent")]
        parent_page: Option<String>,
        /// Properties JSON: `HashMap<String, PropertyValue>`.
        #[arg(long)]
        properties: String,
    },
    /// Update a page's properties / archive / trash state.
    Update {
        /// Page ID or URL.
        id: String,
        /// Properties JSON.
        #[arg(long)]
        properties: Option<String>,
        /// Set `archived` flag.
        #[arg(long)]
        archived: Option<bool>,
        /// Set `in_trash` flag (newer API field).
        #[arg(long)]
        in_trash: Option<bool>,
    },
    /// Archive a page (sugar for `page update --in-trash true`).
    Archive {
        /// Page ID or URL.
        id: String,
    },
}

#[allow(clippy::too_many_lines)]
pub async fn run(cli: &Cli, cmd: &PageCmd) -> Result<(), CliError> {
    match cmd {
        PageCmd::Get { id } => {
            let page_id = PageId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("page id: {e}")))?;
            if cli.check_request {
                emit(&cli.output_options(), &serde_json::json!({
                    "method": "GET",
                    "path": format!("/v1/pages/{page_id}"),
                }))?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let page = client.retrieve_page(&page_id).await?;
            emit(&cli.output_options(), &page)?;
            Ok(())
        }
        PageCmd::Create {
            parent_data_source,
            parent_page,
            properties,
        } => {
            let parent = match (parent_data_source, parent_page) {
                (Some(ds), None) => PageParent::DataSource {
                    data_source_id: DataSourceId::from_url_or_id(ds)
                        .map_err(|e| CliError::Validation(format!("--parent-data-source: {e}")))?,
                },
                (None, Some(p)) => PageParent::Page {
                    page_id: PageId::from_url_or_id(p)
                        .map_err(|e| CliError::Validation(format!("--parent-page: {e}")))?,
                },
                _ => {
                    return Err(CliError::Usage(
                        "exactly one of --parent-data-source or --parent-page required".into(),
                    ));
                }
            };
            let props: HashMap<String, PropertyValue> = serde_json::from_str(properties)
                .map_err(|e| CliError::Validation(format!("--properties: {e}")))?;
            let req = CreatePageRequest {
                parent,
                properties: props,
            };
            if cli.check_request {
                emit(&cli.output_options(), &serde_json::json!({
                    "method": "POST",
                    "path": "/v1/pages",
                    "body": req,
                }))?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let page = client.create_page(&req).await?;
            emit(&cli.output_options(), &page)?;
            Ok(())
        }
        PageCmd::Update {
            id,
            properties,
            archived,
            in_trash,
        } => {
            let page_id = PageId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("page id: {e}")))?;
            let props: HashMap<String, PropertyValue> = properties
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .map_err(|e| CliError::Validation(format!("--properties: {e}")))?
                .unwrap_or_default();
            let req = UpdatePageRequest {
                properties: props,
                archived: *archived,
                in_trash: *in_trash,
            };
            if cli.check_request {
                emit(&cli.output_options(), &serde_json::json!({
                    "method": "PATCH",
                    "path": format!("/v1/pages/{page_id}"),
                    "body": req,
                }))?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let page = client.update_page(&page_id, &req).await?;
            emit(&cli.output_options(), &page)?;
            Ok(())
        }
        PageCmd::Archive { id } => {
            let page_id = PageId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("page id: {e}")))?;
            let req = UpdatePageRequest {
                properties: HashMap::new(),
                archived: None,
                in_trash: Some(true),
            };
            if cli.check_request {
                emit(&cli.output_options(), &serde_json::json!({
                    "method": "PATCH",
                    "path": format!("/v1/pages/{page_id}"),
                    "body": req,
                }))?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let page = client.update_page(&page_id, &req).await?;
            emit(&cli.output_options(), &page)?;
            Ok(())
        }
    }
}
