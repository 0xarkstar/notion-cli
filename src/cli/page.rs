//! `notion-cli page *` — page commands.

use std::collections::HashMap;

use clap::Subcommand;

use crate::api::page::{CreatePageRequest, MoveTarget, MovePageRequest, ParentForMove, PageParent, UpdatePageRequest};
use crate::cli::json_body::{parse_json_body, reject_json_with_bespoke};
use crate::cli::{build_client, Cli, CliError};
use crate::output::emit;
use crate::types::icon::{Cover, Icon};
use crate::types::property::PropertyValue;
use crate::validation::{DataSourceId, PageId};

#[derive(Subcommand, Debug)]
pub enum PageCmd {
    /// Retrieve a page (optionally filtering to specific property IDs).
    Get {
        /// Page ID or URL.
        id: String,
        /// Comma-separated list of property IDs to include in the response.
        /// When set, calls the filtered endpoint and only the listed
        /// properties are hydrated. Use internal property IDs (not display
        /// names). Example: `--properties id1,id2`.
        #[arg(long, value_delimiter = ',')]
        properties: Vec<String>,
    },
    /// Retrieve a single property of a page by property ID.
    /// Supports paginated retrieval for list-valued types (relation,
    /// rollup, people, title, `rich_text`).
    GetProperty {
        /// Page ID or URL.
        page_id: String,
        /// Property ID (internal ID, not display name).
        property_id: String,
        /// Pagination cursor from a previous response's `next_cursor`.
        #[arg(long)]
        cursor: Option<String>,
        /// Results per page (1-100, for list-valued properties).
        #[arg(long)]
        page_size: Option<u8>,
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
        properties: Option<String>,
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
        /// Full `CreatePageRequest` body as JSON (literal, `-` for stdin,
        /// or `@path` for file). Mutually exclusive with all bespoke flags.
        #[arg(long)]
        json: Option<String>,
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
        /// Full `UpdatePageRequest` body as JSON (literal, `-` for stdin,
        /// or `@path` for file). Mutually exclusive with all bespoke flags.
        #[arg(long)]
        json: Option<String>,
    },
    /// Archive a page (sugar for `page update --in-trash true`).
    Archive {
        /// Page ID or URL.
        id: String,
    },
    /// Relocate a page to a new parent (D12). Uses the dedicated
    /// `POST /v1/pages/{id}/move` endpoint — `PATCH /v1/pages/{id}`
    /// does NOT accept parent mutation.
    ///
    /// Exactly one of `--to-page` or `--to-data-source` required.
    /// Restrictions: source must be a regular page (not a database),
    /// integration needs edit access to the new parent, cross-
    /// workspace moves are rejected.
    Move {
        /// Page ID or URL to move.
        id: String,
        /// Move under this parent page (mutually exclusive with
        /// `--to-data-source`).
        #[arg(long)]
        to_page: Option<String>,
        /// Move into this data source (mutually exclusive with
        /// `--to-page`).
        #[arg(long)]
        to_data_source: Option<String>,
        /// Full `MovePageRequest` body as JSON (literal, `-` for stdin,
        /// or `@path` for file). Mutually exclusive with --to-page /
        /// --to-data-source.
        #[arg(long)]
        json: Option<String>,
    },
}

#[allow(clippy::too_many_lines)]
pub async fn run(cli: &Cli, cmd: &PageCmd) -> Result<(), CliError> {
    match cmd {
        PageCmd::Get { id, properties } => {
            let page_id = PageId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("page id: {e}")))?;
            if cli.is_dry_run() {
                if properties.is_empty() {
                    emit(&cli.output_options(), &serde_json::json!({
                        "method": "GET",
                        "path": format!("/v1/pages/{page_id}"),
                    }))?;
                } else {
                    let qs: String = properties
                        .iter()
                        .map(|p| format!("filter_properties={p}"))
                        .collect::<Vec<_>>()
                        .join("&");
                    emit(&cli.output_options(), &serde_json::json!({
                        "method": "GET",
                        "path": format!("/v1/pages/{page_id}?{qs}"),
                    }))?;
                }
                return Ok(());
            }
            let client = build_client(cli)?;
            let page = if properties.is_empty() {
                client.retrieve_page(&page_id).await?
            } else {
                client.retrieve_page_filtered(&page_id, properties).await?
            };
            emit(&cli.output_options(), &page)?;
            Ok(())
        }
        PageCmd::GetProperty {
            page_id,
            property_id,
            cursor,
            page_size,
        } => {
            let pid = PageId::from_url_or_id(page_id)
                .map_err(|e| CliError::Validation(format!("page id: {e}")))?;
            if cli.is_dry_run() {
                let mut path = format!("/v1/pages/{pid}/properties/{property_id}");
                let mut qs: Vec<String> = Vec::new();
                if let Some(c) = cursor {
                    qs.push(format!("start_cursor={c}"));
                }
                if let Some(s) = page_size {
                    qs.push(format!("page_size={s}"));
                }
                if !qs.is_empty() {
                    path.push('?');
                    path.push_str(&qs.join("&"));
                }
                emit(&cli.output_options(), &serde_json::json!({
                    "method": "GET",
                    "path": path,
                }))?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let result = client
                .retrieve_page_property(
                    &pid,
                    property_id,
                    cursor.as_deref(),
                    *page_size,
                )
                .await?;
            emit(&cli.output_options(), &result)?;
            Ok(())
        }
        PageCmd::Create {
            parent_data_source,
            parent_page,
            properties,
            children,
            icon,
            cover,
            json,
        } => {
            let req: CreatePageRequest = if let Some(raw) = json {
                reject_json_with_bespoke(true, &[
                    ("--parent-data-source", parent_data_source.is_some()),
                    ("--parent-page", parent_page.is_some()),
                    ("--properties", properties.is_some()),
                    ("--children", children.is_some()),
                    ("--icon", icon.is_some()),
                    ("--cover", cover.is_some()),
                ])?;
                let val = parse_json_body(raw)?;
                serde_json::from_value(val)
                    .map_err(|e| CliError::Validation(format!("--json body: {e}")))?
            } else {
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
                let props: HashMap<String, PropertyValue> = properties
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()
                    .map_err(|e| CliError::Validation(format!("--properties: {e}")))?
                    .unwrap_or_default();
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
                CreatePageRequest {
                    parent,
                    properties: props,
                    children: child_blocks,
                    icon: create_icon,
                    cover: create_cover,
                }
            };
            if cli.is_dry_run() {
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
            json,
        } => {
            let page_id = PageId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("page id: {e}")))?;
            let req: UpdatePageRequest = if let Some(raw) = json {
                reject_json_with_bespoke(true, &[
                    ("--properties", properties.is_some()),
                    ("--archived", archived.is_some()),
                    ("--in-trash", in_trash.is_some()),
                    ("--icon", icon.is_some()),
                    ("--cover", cover.is_some()),
                ])?;
                let val = parse_json_body(raw)?;
                serde_json::from_value(val)
                    .map_err(|e| CliError::Validation(format!("--json body: {e}")))?
            } else {
                let props: HashMap<String, PropertyValue> = properties
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()
                    .map_err(|e| CliError::Validation(format!("--properties: {e}")))?
                    .unwrap_or_default();
                let icon_patch = parse_icon_flag(icon.as_deref());
                let cover_patch = parse_cover_flag(cover.as_deref())
                    .map_err(|e| CliError::Validation(format!("--cover: {e}")))?;
                UpdatePageRequest {
                    properties: props,
                    archived: *archived,
                    in_trash: *in_trash,
                    icon: icon_patch,
                    cover: cover_patch,
                }
            };
            if cli.is_dry_run() {
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
        PageCmd::Move {
            id,
            to_page,
            to_data_source,
            json,
        } => {
            let page_id = PageId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("page id: {e}")))?;
            // --json path: deserialize full MovePageRequest body.
            if let Some(raw) = json {
                reject_json_with_bespoke(true, &[
                    ("--to-page", to_page.is_some()),
                    ("--to-data-source", to_data_source.is_some()),
                ])?;
                let val = parse_json_body(raw)?;
                let req: MovePageRequest = serde_json::from_value(val)
                    .map_err(|e| CliError::Validation(format!("--json body: {e}")))?;
                if cli.is_dry_run() {
                    emit(
                        &cli.output_options(),
                        &serde_json::json!({
                            "method": "POST",
                            "path": format!("/v1/pages/{page_id}/move"),
                            "body": req,
                        }),
                    )?;
                    return Ok(());
                }
                let client = build_client(cli)?;
                // Reconstruct a MoveTarget from the deserialized parent.
                let target = match req.parent {
                    ParentForMove::Page { page_id: p } => MoveTarget::ToPage(p),
                    ParentForMove::DataSource { data_source_id: ds } => MoveTarget::ToDataSource(ds),
                };
                let page = client.move_page(&page_id, target).await?;
                emit(&cli.output_options(), &page)?;
                return Ok(());
            }
            let target = match (to_page.as_deref(), to_data_source.as_deref()) {
                (Some(p), None) => MoveTarget::ToPage(
                    PageId::from_url_or_id(p)
                        .map_err(|e| CliError::Validation(format!("--to-page: {e}")))?,
                ),
                (None, Some(ds)) => MoveTarget::ToDataSource(
                    DataSourceId::from_url_or_id(ds)
                        .map_err(|e| CliError::Validation(format!("--to-data-source: {e}")))?,
                ),
                _ => {
                    return Err(CliError::Usage(
                        "exactly one of --to-page or --to-data-source required".into(),
                    ));
                }
            };
            if cli.is_dry_run() {
                let parent_json = match &target {
                    MoveTarget::ToPage(p) => serde_json::json!({
                        "type": "page_id", "page_id": p
                    }),
                    MoveTarget::ToDataSource(ds) => serde_json::json!({
                        "type": "data_source_id", "data_source_id": ds
                    }),
                };
                emit(
                    &cli.output_options(),
                    &serde_json::json!({
                        "method": "POST",
                        "path": format!("/v1/pages/{page_id}/move"),
                        "body": { "parent": parent_json },
                    }),
                )?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let page = client.move_page(&page_id, target).await?;
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
            if cli.is_dry_run() {
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
#[allow(clippy::option_option)]
fn parse_icon_flag(value: Option<&str>) -> Option<Option<Icon>> {
    match value {
        None => None,
        Some(v) if v.eq_ignore_ascii_case("none") => Some(None),
        Some(v) => Some(Some(Icon::parse_cli(v))),
    }
}

/// Parse `--cover <value>` tristate for page update. Covers accept
/// URLs only — `"none"` clears, any non-URL value errors.
#[allow(clippy::option_option)]
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
