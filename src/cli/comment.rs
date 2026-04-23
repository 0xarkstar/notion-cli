//! `notion-cli comments *` — CLI-only comments (D10).
//!
//! Not exposed over MCP in v0.3. Notion's comment model is
//! discussion-based (not reply-hierarchy): replies are new comments
//! posted to the same `discussion_id`.

use clap::Subcommand;

use crate::api::comment::{
    CommentParent, CreateCommentRequest, ListCommentsOptions,
};
use crate::cli::{build_client, Cli, CliError};
use crate::output::{emit, emit_stream_end, emit_stream_error, emit_stream_item};
use crate::types::comment::Comment;
use crate::types::rich_text::RichText;
use crate::validation::{BlockId, PageId};

#[derive(Subcommand, Debug)]
pub enum CommentsCmd {
    /// List comments on a page or block. A page ID is also a block
    /// ID — both `--on-page` and `--on-block` accept either, the
    /// distinction is ergonomic.
    List {
        /// Page ID or URL (mutually exclusive with --on-block).
        #[arg(long)]
        on_page: Option<String>,
        /// Block ID or URL (mutually exclusive with --on-page).
        #[arg(long)]
        on_block: Option<String>,
        #[arg(long)]
        page_size: Option<u8>,
        /// Cap on total results after pagination. Omit to collect all.
        #[arg(long)]
        limit: Option<usize>,
        /// Manual pagination cursor (single-page fetch).
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Create a comment.
    ///
    /// Exactly one of `--on-page` (top-level on a page) or
    /// `--in-discussion` (reply inside an existing discussion)
    /// required. Notion's API does not support creating top-level
    /// comments on non-page blocks without an existing discussion.
    Create {
        /// Page ID or URL to post a top-level comment on.
        #[arg(long)]
        on_page: Option<String>,
        /// Discussion ID to reply into.
        #[arg(long)]
        in_discussion: Option<String>,
        /// Plain-text comment body.
        #[arg(long)]
        text: String,
    },
}

pub async fn run(cli: &Cli, cmd: &CommentsCmd) -> Result<(), CliError> {
    match cmd {
        CommentsCmd::List {
            on_page,
            on_block,
            page_size,
            limit,
            cursor,
        } => run_list(cli, on_page.as_deref(), on_block.as_deref(), *page_size, *limit, cursor.clone()).await,
        CommentsCmd::Create {
            on_page,
            in_discussion,
            text,
        } => run_create(cli, on_page.as_deref(), in_discussion.as_deref(), text).await,
    }
}

async fn run_list(
    cli: &Cli,
    on_page: Option<&str>,
    on_block: Option<&str>,
    page_size: Option<u8>,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<(), CliError> {
    let block_id = match (on_page, on_block) {
        (Some(p), None) => BlockId::from_url_or_id(p)
            .or_else(|_| {
                PageId::from_url_or_id(p).map(|pid| {
                    BlockId::parse(pid.as_str()).expect("page id is a valid block id")
                })
            })
            .map_err(|e| CliError::Validation(format!("--on-page: {e}")))?,
        (None, Some(b)) => BlockId::from_url_or_id(b)
            .map_err(|e| CliError::Validation(format!("--on-block: {e}")))?,
        _ => {
            return Err(CliError::Usage(
                "exactly one of --on-page or --on-block required".into(),
            ));
        }
    };
    if cli.is_dry_run() {
        emit(
            &cli.output_options(),
            &serde_json::json!({
                "method": "GET",
                "path": "/v1/comments",
                "query": { "block_id": block_id },
            }),
        )?;
        return Ok(());
    }
    let client = build_client(cli)?;
    let streaming = cli.is_stream();
    let mut opts = ListCommentsOptions {
        block_id: block_id.clone(),
        page_size,
        start_cursor: cursor.clone(),
    };
    let mut collected: Vec<Comment> = Vec::new();
    let mut last_cursor: Option<String> = None;
    loop {
        let resp = match client.list_comments(&opts).await {
            Ok(r) => r,
            Err(e) => {
                if streaming {
                    emit_stream_error(
                        last_cursor.as_deref(),
                        "api_error",
                        &e.to_string(),
                    )?;
                    return Err(CliError::Api(e));
                }
                return Err(CliError::Api(e));
            }
        };
        last_cursor.clone_from(&resp.next_cursor);
        let exhausted = cursor.is_some() || !resp.has_more || resp.next_cursor.is_none();
        for c in resp.results {
            if streaming {
                emit_stream_item(&serde_json::to_value(&c)?)?;
            } else {
                collected.push(c);
            }
            if let Some(cap) = limit {
                if collected.len() >= cap {
                    break;
                }
            }
        }
        if let Some(cap) = limit {
            if collected.len() >= cap {
                break;
            }
        }
        if exhausted {
            break;
        }
        opts.start_cursor = resp.next_cursor;
    }
    if streaming {
        emit_stream_end(None)?;
        return Ok(());
    }
    let out = serde_json::json!({
        "results": collected,
        "has_more": false,
        "next_cursor": serde_json::Value::Null,
    });
    emit(&cli.output_options(), &out)?;
    Ok(())
}

async fn run_create(
    cli: &Cli,
    on_page: Option<&str>,
    in_discussion: Option<&str>,
    text: &str,
) -> Result<(), CliError> {
    let (parent, discussion_id) = match (on_page, in_discussion) {
        (Some(p), None) => {
            let pid = PageId::from_url_or_id(p)
                .map_err(|e| CliError::Validation(format!("--on-page: {e}")))?;
            (Some(CommentParent { page_id: pid }), None)
        }
        (None, Some(d)) => (None, Some(d.to_string())),
        _ => {
            return Err(CliError::Usage(
                "exactly one of --on-page or --in-discussion required".into(),
            ));
        }
    };
    let req = CreateCommentRequest {
        parent,
        discussion_id,
        rich_text: RichText::plain(text),
    };
    if cli.is_dry_run() {
        emit(
            &cli.output_options(),
            &serde_json::json!({
                "method": "POST",
                "path": "/v1/comments",
                "body": req,
            }),
        )?;
        return Ok(());
    }
    let client = build_client(cli)?;
    let comment = client.create_comment(&req).await?;
    emit(&cli.output_options(), &comment)?;
    Ok(())
}
