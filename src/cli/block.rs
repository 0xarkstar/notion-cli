//! `notion-cli block *` — block CRUD commands.

use clap::Subcommand;

use crate::api::block::{parse_children, AppendBlockChildrenRequest, UpdateBlockRequest};
use crate::cli::{build_client, Cli, CliError};
use crate::output::{emit, emit_stream_end, emit_stream_error, emit_stream_item};
use crate::types::block::BlockBody;
use crate::validation::BlockId;

#[derive(Subcommand, Debug)]
pub enum BlockCmd {
    /// Retrieve a single block.
    Get {
        /// Block ID or URL.
        id: String,
    },
    /// List a block's children (paginated).
    List {
        /// Parent block ID (a page ID is also a block ID).
        id: String,
        /// Pagination cursor.
        #[arg(long)]
        start_cursor: Option<String>,
        /// Results per page (1-100).
        #[arg(long)]
        page_size: Option<u8>,
    },
    /// Append children to a parent block.
    Append {
        /// Parent block ID.
        id: String,
        /// JSON array of block bodies, e.g.
        /// `[{"type":"paragraph","paragraph":{"rich_text":[...]}}]`.
        #[arg(long)]
        children: String,
        /// Optional: append after this sibling block ID.
        #[arg(long)]
        after: Option<String>,
    },
    /// Update a block's content or archive state.
    Update {
        /// Block ID.
        id: String,
        /// JSON body: a single block body object matching the block's type.
        #[arg(long)]
        body: Option<String>,
        /// Set `archived` flag.
        #[arg(long)]
        archived: Option<bool>,
        /// Set `in_trash` flag.
        #[arg(long)]
        in_trash: Option<bool>,
    },
    /// Delete (archive) a block.
    Delete {
        /// Block ID.
        id: String,
    },
}

#[allow(clippy::too_many_lines)]
pub async fn run(cli: &Cli, cmd: &BlockCmd) -> Result<(), CliError> {
    match cmd {
        BlockCmd::Get { id } => {
            let block_id = BlockId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("block id: {e}")))?;
            if cli.is_dry_run() {
                emit(&cli.output_options(), &serde_json::json!({
                    "method": "GET",
                    "path": format!("/v1/blocks/{block_id}"),
                }))?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let block = client.retrieve_block(&block_id).await?;
            emit(&cli.output_options(), &block)?;
            Ok(())
        }
        BlockCmd::List { id, start_cursor, page_size } => {
            let block_id = BlockId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("block id: {e}")))?;
            if cli.is_dry_run() {
                let mut qs = url::form_urlencoded::Serializer::new(String::new());
                if let Some(c) = start_cursor {
                    qs.append_pair("start_cursor", c);
                }
                if let Some(p) = page_size {
                    qs.append_pair("page_size", &p.to_string());
                }
                let encoded = qs.finish();
                let suffix = if encoded.is_empty() {
                    String::new()
                } else {
                    format!("?{encoded}")
                };
                emit(&cli.output_options(), &serde_json::json!({
                    "method": "GET",
                    "path": format!("/v1/blocks/{block_id}/children{suffix}"),
                }))?;
                return Ok(());
            }
            let client = build_client(cli)?;
            if cli.is_stream() {
                let mut cur_cursor = start_cursor.clone();
                let mut last_cursor: Option<String> = None;
                loop {
                    let resp = match client
                        .list_block_children(&block_id, cur_cursor.as_deref(), *page_size)
                        .await
                    {
                        Ok(r) => r,
                        Err(e) => {
                            emit_stream_error(
                                last_cursor.as_deref(),
                                "api_error",
                                &e.to_string(),
                            )?;
                            return Err(CliError::Api(e));
                        }
                    };
                    last_cursor.clone_from(&resp.next_cursor);
                    for block in &resp.results {
                        emit_stream_item(&serde_json::to_value(block)?)?;
                    }
                    if resp.is_exhausted() {
                        emit_stream_end(resp.next_cursor.as_deref())?;
                        break;
                    }
                    cur_cursor = resp.next_cursor;
                }
                return Ok(());
            }
            let resp = client
                .list_block_children(&block_id, start_cursor.as_deref(), *page_size)
                .await?;
            emit(&cli.output_options(), &resp)?;
            Ok(())
        }
        BlockCmd::Append { id, children, after } => {
            let block_id = BlockId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("block id: {e}")))?;
            let child_bodies: Vec<BlockBody> = parse_children(children)
                .map_err(|e| CliError::Validation(format!("--children: {e}")))?;
            let after_id = after
                .as_deref()
                .map(BlockId::from_url_or_id)
                .transpose()
                .map_err(|e| CliError::Validation(format!("--after: {e}")))?;
            let req = AppendBlockChildrenRequest {
                children: child_bodies,
                after: after_id,
            };
            if cli.is_dry_run() {
                emit(&cli.output_options(), &serde_json::json!({
                    "method": "PATCH",
                    "path": format!("/v1/blocks/{block_id}/children"),
                    "body": req,
                }))?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let resp = client.append_block_children(&block_id, &req).await?;
            emit(&cli.output_options(), &resp)?;
            Ok(())
        }
        BlockCmd::Update { id, body, archived, in_trash } => {
            let block_id = BlockId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("block id: {e}")))?;
            let body_val: Option<BlockBody> = body
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .map_err(|e| CliError::Validation(format!("--body: {e}")))?;
            let req = UpdateBlockRequest {
                body: body_val,
                archived: *archived,
                in_trash: *in_trash,
            };
            if cli.is_dry_run() {
                emit(&cli.output_options(), &serde_json::json!({
                    "method": "PATCH",
                    "path": format!("/v1/blocks/{block_id}"),
                    "body": req,
                }))?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let block = client.update_block(&block_id, &req).await?;
            emit(&cli.output_options(), &block)?;
            Ok(())
        }
        BlockCmd::Delete { id } => {
            let block_id = BlockId::from_url_or_id(id)
                .map_err(|e| CliError::Validation(format!("block id: {e}")))?;
            if cli.is_dry_run() {
                emit(&cli.output_options(), &serde_json::json!({
                    "method": "DELETE",
                    "path": format!("/v1/blocks/{block_id}"),
                }))?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let block = client.delete_block(&block_id).await?;
            emit(&cli.output_options(), &block)?;
            Ok(())
        }
    }
}
