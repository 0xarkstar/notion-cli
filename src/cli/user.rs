//! `notion-cli users *` — CLI-only user enumeration (D9).
//!
//! Not exposed over MCP in v0.3. Workspace user enumeration is a
//! privacy-adjacent surface; the BlueNode bootstrap didn't require
//! it. Revisit MCP exposure in v0.4 if a concrete agent use case
//! emerges.

use clap::Subcommand;

use crate::api::user::ListUsersOptions;
use crate::cli::{build_client, Cli, CliError};
use crate::output::emit;
use crate::types::user::User;
use crate::validation::UserId;

#[derive(Subcommand, Debug)]
pub enum UsersCmd {
    /// List users in the workspace (auto-paginated by default).
    List {
        /// Page size (1-100). Default 100 (Notion's max).
        #[arg(long)]
        page_size: Option<u8>,
        /// Cap on total results after client-side filtering. Omit
        /// to return all pages.
        #[arg(long)]
        limit: Option<usize>,
        /// Manual pagination cursor (single-page fetch). Mutually
        /// exclusive with auto-pagination.
        #[arg(long)]
        cursor: Option<String>,
        /// Return only bot users.
        #[arg(long, conflicts_with = "human_only")]
        bot_only: bool,
        /// Return only person users.
        #[arg(long)]
        human_only: bool,
    },
    /// Retrieve a single user by ID.
    Get {
        /// User ID (UUID).
        id: String,
    },
}

pub async fn run(cli: &Cli, cmd: &UsersCmd) -> Result<(), CliError> {
    match cmd {
        UsersCmd::List {
            page_size,
            limit,
            cursor,
            bot_only,
            human_only,
        } => {
            if cli.check_request {
                emit(
                    &cli.output_options(),
                    &serde_json::json!({
                        "method": "GET",
                        "path": "/v1/users",
                        "auto_paginate": cursor.is_none(),
                        "client_filter": if *bot_only {
                            "bot_only"
                        } else if *human_only {
                            "human_only"
                        } else {
                            "none"
                        },
                    }),
                )?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let mut opts = ListUsersOptions {
                page_size: *page_size,
                start_cursor: cursor.clone(),
            };
            let mut collected: Vec<User> = Vec::new();
            loop {
                let resp = client.list_users(&opts).await?;
                for u in resp.results {
                    if *bot_only && !u.is_bot() {
                        continue;
                    }
                    if *human_only && !u.is_person() {
                        continue;
                    }
                    collected.push(u);
                    if let Some(cap) = limit {
                        if collected.len() >= *cap {
                            break;
                        }
                    }
                }
                if let Some(cap) = limit {
                    if collected.len() >= *cap {
                        break;
                    }
                }
                // Stop paginating when the caller asked for a specific
                // cursor (single-page fetch) or the server is done.
                if cursor.is_some() || !resp.has_more || resp.next_cursor.is_none() {
                    break;
                }
                opts.start_cursor = resp.next_cursor;
            }
            // Reuse the paginated-response envelope shape on the way
            // out so callers see a consistent structure.
            let out = serde_json::json!({
                "results": collected,
                "has_more": false,
                "next_cursor": serde_json::Value::Null,
            });
            emit(&cli.output_options(), &out)?;
            Ok(())
        }
        UsersCmd::Get { id } => {
            let user_id = UserId::parse(id)
                .map_err(|e| CliError::Validation(format!("user id: {e}")))?;
            if cli.check_request {
                emit(
                    &cli.output_options(),
                    &serde_json::json!({
                        "method": "GET",
                        "path": format!("/v1/users/{user_id}"),
                    }),
                )?;
                return Ok(());
            }
            let client = build_client(cli)?;
            let user = client.retrieve_user(&user_id).await?;
            emit(&cli.output_options(), &user)?;
            Ok(())
        }
    }
}
