//! `notion-cli page *` — page commands.

use std::collections::HashMap;

use clap::Subcommand;

use crate::api::page::{CreatePageRequest, PageParent, UpdatePageRequest};
use crate::cli::{build_client, Cli, CliError};
use crate::output::emit;
use crate::types::icon::{Cover, Icon};
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
        /// Optional JSON array of block bodies for the page body.
        /// Example: `[{"type":"paragraph","paragraph":{"rich_text":[...]}}]`.
        #[arg(long)]
        children: Option<String>,
        /// Icon: emoji literal (e.g. `🚀`) or `http(s)://` URL.
        #[arg(long)]
        icon: Option<String>,
        /// Cover image URL (http/https only).
        #[arg(long)]
        cover: Option<String>,
    },
    /// Update a page's properties / archive / trash state / icon / cover.
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
        /// Icon: emoji / URL to set, `none` to clear, or omit to
        /// leave unchanged (tristate — D11).
        #[arg(long)]
        icon: Option<String>,
        /// Cover URL, `none` to clear, or omit to leave unchanged.
        #[arg(long)]
        cover: Option<String>,
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
            children,
            icon,
            cover,
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
            let child_blocks = children
                .as_deref()
                .map(crate::api::block::parse_children)
                .transpose()
                .map_err(|e| CliError::Validation(format!("--children: {e}")))?
                .unwrap_or_default();
            // On create, `icon` / `cover` can be Set or omitted — no
            // "clear" state (the page doesn't exist yet).
            let create_icon = icon.as_deref().map(Icon::parse_cli);
            let create_cover = cover
                .as_deref()
                .map(parse_cover_url)
                .transpose()
                .map_err(|e| CliError::Validation(format!("--cover: {e}")))?;
            let req = CreatePageRequest {
                parent,
                properties: props,
                children: child_blocks,
                icon: create_icon,
                cover: create_cover,
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
            icon,
            cover,
        } => {
            let page_id = PageId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("page id: {e}")))?;
            let props: HashMap<String, PropertyValue> = properties
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .map_err(|e| CliError::Validation(format!("--properties: {e}")))?
                .unwrap_or_default();
            let icon_patch = parse_icon_flag(icon.as_deref());
            let cover_patch = parse_cover_flag(cover.as_deref())
                .map_err(|e| CliError::Validation(format!("--cover: {e}")))?;
            let req = UpdatePageRequest {
                properties: props,
                archived: *archived,
                in_trash: *in_trash,
                icon: icon_patch,
                cover: cover_patch,
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
                icon: None,
                cover: None,
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

/// Parse `--icon <value>` tristate for page update.
///
/// - absent flag → `None` (leave unchanged)
/// - `--icon none` → `Some(None)` (clear)
/// - `--icon <value>` → `Some(Some(Icon))` (set: URL → external, else emoji)
fn parse_icon_flag(value: Option<&str>) -> Option<Option<Icon>> {
    match value {
        None => None,
        Some(v) if v.eq_ignore_ascii_case("none") => Some(None),
        Some(v) => Some(Some(Icon::parse_cli(v))),
    }
}

/// Parse `--cover <value>` tristate for page update. Covers accept
/// URLs only — `"none"` clears, any non-URL value errors.
fn parse_cover_flag(value: Option<&str>) -> Result<Option<Option<Cover>>, String> {
    match value {
        None => Ok(None),
        Some(v) if v.eq_ignore_ascii_case("none") => Ok(Some(None)),
        Some(v) => Ok(Some(Some(parse_cover_url(v)?))),
    }
}

fn parse_cover_url(value: &str) -> Result<Cover, String> {
    if value.starts_with("http://") || value.starts_with("https://") {
        Ok(Cover::external(value))
    } else {
        Err(format!(
            "must be a URL (http:// or https://), got: {value}"
        ))
    }
}
