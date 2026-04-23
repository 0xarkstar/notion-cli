//! `notion-cli search` — full-text search.

use clap::Args;

use crate::api::search::SearchRequest;
use crate::cli::{build_client, Cli, CliError};
use crate::output::{emit, emit_stream_end, emit_stream_error, emit_stream_item};

#[derive(Args, Debug)]
pub struct SearchArgs {
    /// Query string (default: empty, returns everything the
    /// integration has access to).
    pub query: Option<String>,
    /// Filter JSON (e.g. `{"property":"object","value":"page"}`).
    #[arg(long)]
    pub filter: Option<String>,
    /// Sort JSON.
    #[arg(long)]
    pub sort: Option<String>,
    /// Pagination cursor.
    #[arg(long)]
    pub start_cursor: Option<String>,
    /// Results per page (1-100).
    #[arg(long)]
    pub page_size: Option<u8>,
}

pub async fn run(cli: &Cli, args: &SearchArgs) -> Result<(), CliError> {
    let filter_val = args
        .filter
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| CliError::Validation(format!("--filter: {e}")))?;
    let sort_val = args
        .sort
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| CliError::Validation(format!("--sort: {e}")))?;
    let req = SearchRequest {
        query: args.query.clone(),
        filter: filter_val,
        sort: sort_val,
        start_cursor: args.start_cursor.clone(),
        page_size: args.page_size,
    };
    if cli.is_dry_run() {
        emit(&cli.output_options(), &serde_json::json!({
            "method": "POST",
            "path": "/v1/search",
            "body": req,
        }))?;
        return Ok(());
    }
    let client = build_client(cli)?;
    if cli.is_stream() {
        let mut cur_req = req;
        let mut last_cursor: Option<String> = None;
        loop {
            let resp = match client.search(&cur_req).await {
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
            for item in &resp.results {
                emit_stream_item(item)?;
            }
            if resp.is_exhausted() {
                emit_stream_end(resp.next_cursor.as_deref())?;
                break;
            }
            cur_req = SearchRequest {
                start_cursor: resp.next_cursor,
                ..cur_req
            };
        }
        return Ok(());
    }
    let resp = client.search(&req).await?;
    emit(&cli.output_options(), &resp)?;
    Ok(())
}
