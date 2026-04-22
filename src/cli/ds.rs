//! `notion-cli ds *` — data source commands (query, create, retrieve, update).
//!
//! `ds create` is the-bug endpoint — what the upstream
//! `@notionhq/notion-mcp-server` gets wrong on API 2025-09-03+.
//! `ds update` (v0.3) adds single-delta schema mutation per D2.

use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;

use clap::{Subcommand, ValueEnum};

use crate::api::data_source::{
    CreateDataSourceParent, CreateDataSourceRequest, QueryDataSourceRequest,
    RelationDirection, SelectKind, UpdateDataSourceRequest,
};
use crate::cli::{build_client, Cli, CliError};
use crate::output::emit;
use crate::types::common::SelectOption;
use crate::types::property_schema::PropertySchema;
use crate::types::rich_text::RichText;
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
    /// Mutate a data source's schema (single-delta per invocation).
    ///
    /// See `notion-cli ds update add-property --help` for the per-op
    /// flag reference. Destructive ops (remove-property) require
    /// `--yes` in non-TTY contexts (D1).
    #[command(subcommand)]
    Update(UpdateCmd),
    /// Add a relation property — convenience wrapper over `ds update`.
    ///
    /// Exactly one direction flag required: `--backlink <name>` for
    /// two-way (`dual_property`), `--one-way` for `single_property`,
    /// or `--self` for self-referential. Pre-flight: GET on the
    /// target DS to verify it exists and is shared with the
    /// integration (skipped when `--self`).
    AddRelation {
        /// Source data source ID or URL.
        id: String,
        /// Name of the new relation property on the source.
        #[arg(long)]
        name: String,
        /// Target data source ID or URL. Required unless `--self`.
        #[arg(long)]
        target: Option<String>,
        /// Two-way relation: Notion creates a reciprocal property on
        /// the target with this name. Mutually exclusive with
        /// `--one-way` and `--self`.
        #[arg(long)]
        backlink: Option<String>,
        /// One-way relation: no backlink is created on the target.
        #[arg(long)]
        one_way: bool,
        /// Self-referential relation (source == target).
        #[arg(long = "self")]
        self_: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum UpdateCmd {
    /// Add a new property to the schema.
    AddProperty {
        /// Data source ID or URL.
        id: String,
        /// New property name.
        #[arg(long)]
        name: String,
        /// Property schema as inline JSON. Example:
        /// `--schema '{"type":"select","select":{"options":[{"name":"High"}]}}'`.
        #[arg(long)]
        schema: String,
    },
    /// Remove a property from the schema. DESTRUCTIVE.
    RemoveProperty {
        /// Data source ID or URL.
        id: String,
        /// Property name to remove.
        #[arg(long)]
        name: String,
        /// Confirm destructive removal. Required in non-TTY contexts
        /// (agents, scripts); interactive TTY prompt is wired in D1.
        #[arg(long)]
        yes: bool,
    },
    /// Rename a property.
    RenameProperty {
        /// Data source ID or URL.
        id: String,
        /// Current property name.
        #[arg(long)]
        from: String,
        /// New property name.
        #[arg(long)]
        to: String,
    },
    /// Append an option to a select / multi-select / status property.
    /// Notion merges by option name — existing options are preserved.
    AddOption {
        /// Data source ID or URL.
        id: String,
        /// Target property name.
        #[arg(long)]
        property: String,
        /// Property kind.
        #[arg(long, value_enum, default_value_t = SelectKindArg::Select)]
        kind: SelectKindArg,
        /// Option name.
        #[arg(long)]
        name: String,
        /// Optional colour (e.g. `blue`, `red`, `default`).
        #[arg(long)]
        color: Option<String>,
    },
    /// Bulk update (non-atomic escape hatch). Reads a JSON file
    /// containing an `UpdateDataSourceRequest` body. Partial failure
    /// leaves the DS in mixed state — caller accepts that.
    Bulk {
        /// Data source ID or URL.
        id: String,
        /// Path to JSON body file.
        #[arg(long)]
        body: PathBuf,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum SelectKindArg {
    Select,
    MultiSelect,
    Status,
}

impl From<SelectKindArg> for SelectKind {
    fn from(k: SelectKindArg) -> Self {
        match k {
            SelectKindArg::Select => SelectKind::Select,
            SelectKindArg::MultiSelect => SelectKind::MultiSelect,
            SelectKindArg::Status => SelectKind::Status,
        }
    }
}

#[allow(clippy::too_many_lines)]
pub async fn run(cli: &Cli, cmd: &DsCmd) -> Result<(), CliError> {
    match cmd {
        DsCmd::Get { id } => {
            let ds_id = DataSourceId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("data source id: {e}")))?;
            if cli.check_request {
                emit(
                    &cli.output_options(),
                    &serde_json::json!({
                        "method": "GET",
                        "path": format!("/v1/data_sources/{ds_id}"),
                    }),
                )?;
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
                emit(
                    &cli.output_options(),
                    &serde_json::json!({
                        "method": "POST",
                        "path": format!("/v1/data_sources/{ds_id}/query"),
                        "body": req,
                    }),
                )?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let resp = client.query_data_source(&ds_id, &req).await?;
            emit(&cli.output_options(), &resp)?;
            Ok(())
        }
        DsCmd::Create {
            parent,
            title,
            properties,
        } => {
            let db_id = DatabaseId::from_url_or_id(parent)
                .map_err(|e| CliError::Validation(format!("--parent: {e}")))?;
            let props: serde_json::Value = serde_json::from_str(properties)
                .map_err(|e| CliError::Validation(format!("--properties: {e}")))?;
            let title_vec = title
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(RichText::plain)
                .unwrap_or_default();
            let req = CreateDataSourceRequest {
                parent: CreateDataSourceParent::database(db_id),
                title: title_vec,
                properties: props,
            };
            if cli.check_request {
                emit(
                    &cli.output_options(),
                    &serde_json::json!({
                        "method": "POST",
                        "path": "/v1/data_sources",
                        "body": req,
                    }),
                )?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let ds = client.create_data_source(&req).await?;
            emit(&cli.output_options(), &ds)?;
            Ok(())
        }
        DsCmd::Update(sub) => run_update(cli, sub).await,
        DsCmd::AddRelation {
            id,
            name,
            target,
            backlink,
            one_way,
            self_,
        } => {
            run_add_relation(cli, id, name, target.as_deref(), backlink.as_deref(), *one_way, *self_)
                .await
        }
    }
}

async fn run_add_relation(
    cli: &Cli,
    src_id: &str,
    name: &str,
    target: Option<&str>,
    backlink: Option<&str>,
    one_way: bool,
    self_: bool,
) -> Result<(), CliError> {
    let direction_count =
        usize::from(backlink.is_some()) + usize::from(one_way) + usize::from(self_);
    if direction_count != 1 {
        return Err(CliError::Usage(
            "exactly one of --backlink <name>, --one-way, or --self required".into(),
        ));
    }

    let src_ds = parse_ds_id(src_id)?;
    let target_ds = if self_ {
        if let Some(t) = target {
            let parsed = DataSourceId::from_url_or_id(t)
                .map_err(|e| CliError::Validation(format!("--target: {e}")))?;
            if parsed.as_str() != src_ds.as_str() {
                return Err(CliError::Usage(
                    "--self with --target set to a different id — drop --target or --self".into(),
                ));
            }
            parsed
        } else {
            src_ds.clone()
        }
    } else {
        let t = target.ok_or_else(|| {
            CliError::Usage("--target <ds_id> required (unless --self)".into())
        })?;
        DataSourceId::from_url_or_id(t)
            .map_err(|e| CliError::Validation(format!("--target: {e}")))?
    };

    let direction = if let Some(b) = backlink {
        RelationDirection::Dual(b.to_string())
    } else {
        // one_way or self without backlink.
        RelationDirection::OneWay
    };

    let req = UpdateDataSourceRequest::add_relation_property(name, target_ds.clone(), direction);

    if cli.check_request {
        emit(
            &cli.output_options(),
            &serde_json::json!({
                "method": "PATCH",
                "path": format!("/v1/data_sources/{src_ds}"),
                "body": req,
                "preflight": {
                    "method": "GET",
                    "path": format!("/v1/data_sources/{target_ds}"),
                    "skipped_when_self": self_,
                }
            }),
        )?;
        return Ok(());
    }

    let client = build_client(cli)?;
    // Pre-flight: verify target exists + is shared with integration.
    // Skip when --self (same DS as source we're about to PATCH).
    if !self_ {
        client
            .retrieve_data_source(&target_ds)
            .await
            .map_err(CliError::Api)?;
    }
    let ds = client.update_data_source(&src_ds, &req).await?;
    emit(&cli.output_options(), &ds)?;
    Ok(())
}

async fn run_update(cli: &Cli, cmd: &UpdateCmd) -> Result<(), CliError> {
    let (ds_id, req) = build_update(cmd)?;
    if cli.check_request {
        emit(
            &cli.output_options(),
            &serde_json::json!({
                "method": "PATCH",
                "path": format!("/v1/data_sources/{ds_id}"),
                "body": req,
            }),
        )?;
        return Ok(());
    }
    let client = build_client(cli)?;
    let ds = client.update_data_source(&ds_id, &req).await?;
    emit(&cli.output_options(), &ds)?;
    Ok(())
}

fn build_update(cmd: &UpdateCmd) -> Result<(DataSourceId, UpdateDataSourceRequest), CliError> {
    match cmd {
        UpdateCmd::AddProperty { id, name, schema } => {
            let ds_id = parse_ds_id(id)?;
            let parsed: PropertySchema = serde_json::from_str(schema)
                .map_err(|e| CliError::Validation(format!("--schema: {e}")))?;
            let req = UpdateDataSourceRequest::add_property(name, &parsed)
                .map_err(|e| CliError::Validation(format!("build add_property: {e}")))?;
            Ok((ds_id, req))
        }
        UpdateCmd::RemoveProperty { id, name, yes } => {
            if !*yes && !confirm_destructive_tty(
                &format!("remove property '{name}' from this data source"),
            )? {
                return Err(CliError::Validation(format!(
                    "remove-property '{name}' is destructive; pass --yes to \
                     confirm (non-TTY) or answer 'y' at the interactive prompt"
                )));
            }
            let ds_id = parse_ds_id(id)?;
            Ok((ds_id, UpdateDataSourceRequest::remove_property(name)))
        }
        UpdateCmd::RenameProperty { id, from, to } => {
            let ds_id = parse_ds_id(id)?;
            Ok((ds_id, UpdateDataSourceRequest::rename_property(from, to)))
        }
        UpdateCmd::AddOption {
            id,
            property,
            kind,
            name,
            color,
        } => {
            let ds_id = parse_ds_id(id)?;
            let option = SelectOption {
                id: None,
                name: name.clone(),
                color: parse_color(color.as_deref())
                    .map_err(|e| CliError::Validation(format!("--color: {e}")))?,
            };
            let req = UpdateDataSourceRequest::add_option(property, SelectKind::from(*kind), option);
            Ok((ds_id, req))
        }
        UpdateCmd::Bulk { id, body } => {
            let ds_id = parse_ds_id(id)?;
            let text = std::fs::read_to_string(body).map_err(|e| {
                CliError::Validation(format!("--body {}: {e}", body.display()))
            })?;
            let json: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| CliError::Validation(format!("--body JSON: {e}")))?;
            let req = UpdateDataSourceRequest::from_bulk(&json)
                .map_err(|e| CliError::Validation(format!("bulk: {e}")))?;
            Ok((ds_id, req))
        }
    }
}

fn parse_ds_id(s: &str) -> Result<DataSourceId, CliError> {
    DataSourceId::from_url_or_id(s)
        .map_err(|e| CliError::Validation(format!("data source id: {e}")))
}

/// TTY-aware destructive confirmation (D1).
///
/// - Non-TTY (agent, script, pipe): returns `Ok(false)` — caller
///   should error with `CliError::Validation` (exit 2).
/// - TTY: prompts `"{action} [y/N]: "` on stderr, reads stdin; any
///   response beginning with `y` or `Y` accepts.
fn confirm_destructive_tty(action: &str) -> Result<bool, CliError> {
    if !std::io::stdin().is_terminal() {
        return Ok(false);
    }
    let mut err = std::io::stderr();
    write!(err, "About to {action}. Proceed? [y/N]: ").map_err(CliError::Io)?;
    err.flush().map_err(CliError::Io)?;
    let mut input = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut input)
        .map_err(CliError::Io)?;
    Ok(matches!(input.trim().chars().next(), Some('y' | 'Y')))
}

fn parse_color(c: Option<&str>) -> Result<Option<crate::types::common::Color>, String> {
    let Some(c) = c else { return Ok(None) };
    let json = serde_json::json!(c);
    let color: crate::types::common::Color = serde_json::from_value(json)
        .map_err(|e| format!("'{c}' is not a valid color: {e}"))?;
    Ok(Some(color))
}
