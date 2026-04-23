//! `notion-cli db *` — database container commands.

use std::collections::HashMap;
use std::path::PathBuf;

use clap::Subcommand;

use crate::api::database::{
    CreateDatabaseParent, CreateDatabaseRequest, DatabaseParentUpdate, InitialDataSource,
    UpdateDatabaseRequest,
};
use crate::cli::json_body::{parse_json_body, reject_json_with_bespoke};
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
        parent_page: Option<String>,
        /// Database title (plain text).
        #[arg(long)]
        title: Option<String>,
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
        schema: Option<PathBuf>,
        /// Full `CreateDatabaseRequest` body as JSON (literal, `-` for
        /// stdin, or `@path` for file). Mutually exclusive with all
        /// bespoke flags.
        #[arg(long)]
        json: Option<String>,
    },
    /// Update a database container: metadata and/or parent (move to page or workspace).
    ///
    /// `--to-page <id>` and `--to-workspace` are mutually exclusive.
    /// `--to-workspace` requires an OAuth user token (integration tokens
    /// typically 403).
    ///
    /// Tristate for icon/cover: omit flag → leave unchanged;
    /// `--icon-clear` / `--cover-clear` → clear; `--icon <v>` → set.
    Update {
        /// Database ID or URL.
        id: String,
        #[arg(long, conflicts_with = "to_workspace")]
        to_page: Option<String>,
        #[arg(long)]
        to_workspace: bool,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long, conflicts_with = "icon_clear")]
        icon: Option<String>,
        #[arg(long)]
        icon_clear: bool,
        #[arg(long, conflicts_with = "cover_clear")]
        cover: Option<String>,
        #[arg(long)]
        cover_clear: bool,
        #[arg(long)]
        inline: Option<bool>,
        #[arg(long)]
        is_locked: Option<bool>,
        #[arg(long)]
        in_trash: Option<bool>,
        /// Full `UpdateDatabaseRequest` body as JSON (literal, `-` for
        /// stdin, or `@path` for file). Mutually exclusive with all
        /// bespoke flags.
        #[arg(long)]
        json: Option<String>,
    },
}

#[allow(clippy::too_many_lines)]
pub async fn run(cli: &Cli, cmd: &DbCmd) -> Result<(), CliError> {
    match cmd {
        DbCmd::Get { id } => {
            let db_id = DatabaseId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("database id: {e}")))?;
            if cli.is_dry_run() {
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
            json,
        } => {
            let req: CreateDatabaseRequest = if let Some(raw) = json {
                reject_json_with_bespoke(true, &[
                    ("--parent-page", parent_page.is_some()),
                    ("--title", title.is_some()),
                    ("--icon", icon.is_some()),
                    ("--cover", cover.is_some()),
                    ("--inline", *inline),
                    ("--schema", schema.is_some()),
                ])?;
                let val = parse_json_body(raw)?;
                serde_json::from_value(val)
                    .map_err(|e| CliError::Validation(format!("--json body: {e}")))?
            } else {
                let parent_page_str = parent_page.as_deref().ok_or_else(|| {
                    CliError::Usage("--parent-page required (or use --json)".into())
                })?;
                let title_str = title.as_deref().ok_or_else(|| {
                    CliError::Usage("--title required (or use --json)".into())
                })?;
                let schema_path = schema.as_ref().ok_or_else(|| {
                    CliError::Usage("--schema required (or use --json)".into())
                })?;
                let parent_id = PageId::from_url_or_id(parent_page_str)
                    .map_err(|e| CliError::Validation(format!("--parent-page: {e}")))?;
                let schema_text = std::fs::read_to_string(schema_path).map_err(|e| {
                    CliError::Validation(format!("--schema {}: {e}", schema_path.display()))
                })?;
                let properties: HashMap<String, PropertySchema> =
                    serde_json::from_str(&schema_text).map_err(|e| {
                        CliError::Validation(format!("--schema JSON: {e}"))
                    })?;
                CreateDatabaseRequest {
                    parent: CreateDatabaseParent::page(parent_id),
                    title: RichText::plain(title_str),
                    initial_data_source: InitialDataSource { properties },
                    icon: icon.as_deref().map(Icon::parse_cli),
                    cover: cover.as_deref().map(Cover::external),
                    is_inline: if *inline { Some(true) } else { None },
                }
            };
            req.validate_local()
                .map_err(CliError::Validation)?;
            if cli.is_dry_run() {
                if cli.is_cost_preview() {
                    let estimate = crate::observability::cost::CostEstimate::single("POST /v1/databases");
                    emit(
                        &cli.output_options(),
                        &serde_json::json!({
                            "method": "POST",
                            "path": "/v1/databases",
                            "body": req,
                            "estimate": estimate,
                        }),
                    )?;
                } else {
                    emit(
                        &cli.output_options(),
                        &serde_json::json!({
                            "method": "POST",
                            "path": "/v1/databases",
                            "body": req,
                        }),
                    )?;
                }
                return Ok(());
            }
            let client = build_client(cli)?;
            let db = client.create_database(&req).await?;
            emit(&cli.output_options(), &db)?;
            Ok(())
        }
        DbCmd::Update {
            id,
            to_page,
            to_workspace,
            title,
            description,
            icon,
            icon_clear,
            cover,
            cover_clear,
            inline,
            is_locked,
            in_trash,
            json,
        } => {
            let db_id = DatabaseId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("database id: {e}")))?;
            let req: UpdateDatabaseRequest = if let Some(raw) = json {
                reject_json_with_bespoke(true, &[
                    ("--to-page", to_page.is_some()),
                    ("--to-workspace", *to_workspace),
                    ("--title", title.is_some()),
                    ("--description", description.is_some()),
                    ("--icon", icon.is_some()),
                    ("--icon-clear", *icon_clear),
                    ("--cover", cover.is_some()),
                    ("--cover-clear", *cover_clear),
                    ("--inline", inline.is_some()),
                    ("--is-locked", is_locked.is_some()),
                    ("--in-trash", in_trash.is_some()),
                ])?;
                let val = parse_json_body(raw)?;
                serde_json::from_value(val)
                    .map_err(|e| CliError::Validation(format!("--json body: {e}")))?
            } else {
                if to_page.is_some() && *to_workspace {
                    return Err(CliError::Usage(
                        "--to-page and --to-workspace are mutually exclusive".into(),
                    ));
                }
                let mut r = UpdateDatabaseRequest::default();
                if let Some(pid) = to_page {
                    let parent = PageId::from_url_or_id(pid)
                        .map_err(|e| CliError::Validation(format!("--to-page: {e}")))?;
                    r.parent = Some(DatabaseParentUpdate::page(parent));
                } else if *to_workspace {
                    r.parent = Some(DatabaseParentUpdate::workspace());
                }
                if let Some(t) = title {
                    r.title = Some(RichText::plain(t));
                }
                if let Some(d) = description {
                    r.description = Some(RichText::plain(d));
                }
                if *icon_clear {
                    r.icon = Some(None);
                } else if let Some(v) = icon {
                    r.icon = Some(Some(Icon::parse_cli(v)));
                }
                if *cover_clear {
                    r.cover = Some(None);
                } else if let Some(v) = cover {
                    if !v.starts_with("http://") && !v.starts_with("https://") {
                        return Err(CliError::Validation(
                            "--cover must be a URL (http:// or https://)".into(),
                        ));
                    }
                    r.cover = Some(Some(Cover::external(v)));
                }
                r.is_inline = *inline;
                r.is_locked = *is_locked;
                r.in_trash = *in_trash;
                r
            };
            if req.is_empty() {
                return Err(CliError::Usage(
                    "no update fields provided — see --help".into(),
                ));
            }
            if cli.is_dry_run() {
                if cli.is_cost_preview() {
                    let estimate = crate::observability::cost::CostEstimate::single(
                        &format!("PATCH /v1/databases/{db_id}"),
                    );
                    emit(
                        &cli.output_options(),
                        &serde_json::json!({
                            "method": "PATCH",
                            "path": format!("/v1/databases/{db_id}"),
                            "body": req,
                            "estimate": estimate,
                        }),
                    )?;
                } else {
                    emit(
                        &cli.output_options(),
                        &serde_json::json!({
                            "method": "PATCH",
                            "path": format!("/v1/databases/{db_id}"),
                            "body": req,
                        }),
                    )?;
                }
                return Ok(());
            }
            let client = build_client(cli)?;
            let db = client.update_database(&db_id, &req).await?;
            emit(&cli.output_options(), &db)?;
            Ok(())
        }
    }
}
