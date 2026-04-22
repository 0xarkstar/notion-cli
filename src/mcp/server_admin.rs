//! Admin MCP tier — 12 write tools + admin lifecycle ops.
//!
//! # Opt-in via `--allow-admin`
//!
//! Admin ops (`db create`, `ds update`, `ds add-relation`, `page move`)
//! are gated behind a separate flag from `--allow-write`. This is
//! **tool-exposure policy**, NOT a security boundary (D3): an agent
//! with admin-scoped integration token + code execution can hit the
//! API directly. What the flag actually provides:
//!
//! 1. **Prompt-injection attenuation**: admin tools absent from the
//!    agent's tool menu → excluded from its planning surface.
//! 2. **Accidental-action prevention**: default Hermes profiles
//!    expose no admin tools → operator can't fat-finger schema
//!    mutation through a read-or-write-only agent.
//!
//! # Module boundary invariant (D5)
//!
//! Admin-only tool declarations live ONLY in this file. The D13
//! snapshot regression test asserts `tools/list` returns the exact
//! expected set per tier — cross-tier drift trips the test.
//!
//! # v0.3 scaffold status
//!
//! Currently exposes the same 12 tools as `server_write.rs`. Admin
//! tools land per-command:
//! - `db_create` (task 18)
//! - `ds_update` (task 19)
//! - `ds_add_relation` (task 20)
//! - `page_move` (task 22)
//!
//! `users list/get` and `comments list/create` are CLI-only
//! (D9/D10) — do NOT add them here.

use std::path::PathBuf;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::transport::stdio;
use rmcp::{tool, tool_router, ErrorData, ServiceExt};

use crate::api::NotionClient;
use crate::mcp::audit::AuditLog;
use crate::mcp::common::{to_result, Inner};
use crate::mcp::handlers;
use crate::mcp::params::{
    AppendBlockChildrenParams, CreateDataSourceParams, CreatePageParams, DbCreateParams,
    DeleteBlockParams, DsAddRelationParams, DsUpdateParams, GetBlockParams,
    GetDataSourceParams, GetPageParams, ListBlockChildrenParams, PageMoveParams,
    QueryDataSourceParams, SearchParams, UpdateBlockParams, UpdatePageParams,
};

#[derive(Clone)]
pub struct NotionAdmin {
    inner: std::sync::Arc<Inner>,
}

#[tool_router(server_handler)]
impl NotionAdmin {
    // --- reads (same as RO/Write tiers) -----------------------------

    #[tool(name = "get_page", description = "Retrieve a Notion page by ID.")]
    async fn get_page(
        &self,
        params: Parameters<GetPageParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(to_result(&handlers::get_page(&self.inner.client, params.0).await?))
    }

    #[tool(
        name = "get_data_source",
        description = "Retrieve a Notion data source (schema + metadata) by ID."
    )]
    async fn get_data_source(
        &self,
        params: Parameters<GetDataSourceParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(to_result(&handlers::get_data_source(&self.inner.client, params.0).await?))
    }

    #[tool(
        name = "query_data_source",
        description = "Query pages inside a data source with optional filter, sort, and pagination."
    )]
    async fn query_data_source(
        &self,
        params: Parameters<QueryDataSourceParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(to_result(&handlers::query_data_source(&self.inner.client, params.0).await?))
    }

    #[tool(
        name = "search",
        description = "Full-text search across Notion pages and data sources."
    )]
    async fn search(
        &self,
        params: Parameters<SearchParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(to_result(&handlers::search(&self.inner.client, params.0).await?))
    }

    #[tool(
        name = "get_block",
        description = "Retrieve a single Notion block by ID."
    )]
    async fn get_block(
        &self,
        params: Parameters<GetBlockParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(to_result(&handlers::get_block(&self.inner.client, params.0).await?))
    }

    #[tool(
        name = "list_block_children",
        description = "List child blocks of a parent block (a page ID is also a block ID). Paginated."
    )]
    async fn list_block_children(
        &self,
        params: Parameters<ListBlockChildrenParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(to_result(
            &handlers::list_block_children(&self.inner.client, params.0).await?,
        ))
    }

    // --- writes (same as Write tier) --------------------------------

    #[tool(
        name = "create_page",
        description = "Create a Notion page under a data source or another page. Write operation — audited."
    )]
    async fn create_page(
        &self,
        params: Parameters<CreatePageParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let target = params
            .0
            .parent_data_source_id
            .clone()
            .or_else(|| params.0.parent_page_id.clone());
        let result = handlers::create_page(&self.inner.client, params.0).await;
        self.inner.audit.record(
            "create_page",
            target.as_deref(),
            result.as_ref().map(|_| ()).map_err(|e| e.message.as_ref()),
        );
        Ok(to_result(&result?))
    }

    #[tool(
        name = "update_page",
        description = "Update a page's properties, archive, or trash state. Write operation — audited."
    )]
    async fn update_page(
        &self,
        params: Parameters<UpdatePageParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let target = params.0.page_id.clone();
        let result = handlers::update_page(&self.inner.client, params.0).await;
        self.inner.audit.record(
            "update_page",
            Some(&target),
            result.as_ref().map(|_| ()).map_err(|e| e.message.as_ref()),
        );
        Ok(to_result(&result?))
    }

    #[tool(
        name = "create_data_source",
        description = "Create a new data source inside a database container (Notion API 2025-09-03+). Write operation — audited."
    )]
    async fn create_data_source(
        &self,
        params: Parameters<CreateDataSourceParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let target = params.0.parent_database_id.clone();
        let result = handlers::create_data_source(&self.inner.client, params.0).await;
        self.inner.audit.record(
            "create_data_source",
            Some(&target),
            result.as_ref().map(|_| ()).map_err(|e| e.message.as_ref()),
        );
        Ok(to_result(&result?))
    }

    #[tool(
        name = "append_block_children",
        description = "Append new child blocks to a parent block. Write operation — audited."
    )]
    async fn append_block_children(
        &self,
        params: Parameters<AppendBlockChildrenParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let target = params.0.block_id.clone();
        let result = handlers::append_block_children(&self.inner.client, params.0).await;
        self.inner.audit.record(
            "append_block_children",
            Some(&target),
            result.as_ref().map(|_| ()).map_err(|e| e.message.as_ref()),
        );
        Ok(to_result(&result?))
    }

    #[tool(
        name = "update_block",
        description = "Update a block's content, archive, or trash state. Write operation — audited."
    )]
    async fn update_block(
        &self,
        params: Parameters<UpdateBlockParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let target = params.0.block_id.clone();
        let result = handlers::update_block(&self.inner.client, params.0).await;
        self.inner.audit.record(
            "update_block",
            Some(&target),
            result.as_ref().map(|_| ()).map_err(|e| e.message.as_ref()),
        );
        Ok(to_result(&result?))
    }

    #[tool(
        name = "delete_block",
        description = "Archive (soft-delete) a block. Write operation — audited."
    )]
    async fn delete_block(
        &self,
        params: Parameters<DeleteBlockParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let target = params.0.block_id.clone();
        let result = handlers::delete_block(&self.inner.client, params.0).await;
        self.inner.audit.record(
            "delete_block",
            Some(&target),
            result.as_ref().map(|_| ()).map_err(|e| e.message.as_ref()),
        );
        Ok(to_result(&result?))
    }

    // --- admin lifecycle ops ---------------------------------------
    //
    // Reserved for:
    //   - ds_update          (task 19, #2)
    //   - ds_add_relation    (task 20, #3)
    //   - page_move          (task 22, #4)
    //
    // DO NOT add users/comments here — those are CLI-only in v0.3
    // (D9/D10). Move to v0.4 if real agent demand emerges.

    #[tool(
        name = "db_create",
        description = "Create a new database container under a parent page with an initial data-source schema. Admin operation — audited to NOTION_CLI_ADMIN_LOG. `properties` must include at least one `title`-typed entry."
    )]
    async fn db_create(
        &self,
        params: Parameters<DbCreateParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let target = params.0.parent_page_id.clone();
        let result = handlers::db_create(&self.inner.client, params.0).await;
        self.inner.audit.record_admin(
            "db_create",
            Some(&target),
            result.as_ref().map(|_| ()).map_err(|e| e.message.as_ref()),
        );
        Ok(to_result(&result?))
    }

    #[tool(
        name = "ds_update",
        description = "Mutate a data source's schema. `action` dispatches: add_property, remove_property (destructive — requires confirm=true AND NOTION_CLI_ADMIN_CONFIRMED=1 env), rename_property, add_option, bulk (non-atomic escape). Single-delta default per invocation. Admin operation — audited to NOTION_CLI_ADMIN_LOG."
    )]
    async fn ds_update(
        &self,
        params: Parameters<DsUpdateParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let target = params.0.data_source_id.clone();
        let action = params.0.action.clone();
        let result = handlers::ds_update(&self.inner.client, params.0).await;
        self.inner.audit.record_admin(
            &format!("ds_update:{action}"),
            Some(&target),
            result.as_ref().map(|_| ()).map_err(|e| e.message.as_ref()),
        );
        Ok(to_result(&result?))
    }

    #[tool(
        name = "ds_add_relation",
        description = "Add a relation property to a data source. Convenience wrapper over ds_update — generates correct dual_property/single_property shape with data_source_id (not database_id). Exactly one of `backlink` (two-way with named reciprocal property), `one_way` (no backlink), or `self` (self-referential, same DS as source) required. Pre-flight: GET on target verifies existence + integration sharing. Admin operation — audited to NOTION_CLI_ADMIN_LOG."
    )]
    async fn ds_add_relation(
        &self,
        params: Parameters<DsAddRelationParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let target = params.0.source_data_source_id.clone();
        let result = handlers::ds_add_relation(&self.inner.client, params.0).await;
        self.inner.audit.record_admin(
            "ds_add_relation",
            Some(&target),
            result.as_ref().map(|_| ()).map_err(|e| e.message.as_ref()),
        );
        Ok(to_result(&result?))
    }

    #[tool(
        name = "page_move",
        description = "Relocate a page to a new parent. Uses POST /v1/pages/{id}/move — the dedicated endpoint introduced 2026-01-15. PATCH does not accept parent mutation. Exactly one of target_page_id or target_data_source_id required. Restrictions: source must be a regular page (not database), integration needs edit access to new parent, cross-workspace rejected. Admin operation — audited to NOTION_CLI_ADMIN_LOG."
    )]
    async fn page_move(
        &self,
        params: Parameters<PageMoveParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let target = params.0.page_id.clone();
        let result = handlers::page_move(&self.inner.client, params.0).await;
        self.inner.audit.record_admin(
            "page_move",
            Some(&target),
            result.as_ref().map(|_| ()).map_err(|e| e.message.as_ref()),
        );
        Ok(to_result(&result?))
    }
}

/// Entry point for the admin MCP tier.
///
/// # Parameters
///
/// - `audit_log_path`: JSONL sink for runtime write ops (same as Write tier).
/// - `admin_log_path`: JSONL sink for admin lifecycle ops (D6).
///   Wired independently of `audit_log_path` so operators can
///   grep-split agent activity vs structural mutation.
pub async fn run_with_admin(
    client: NotionClient,
    audit_log_path: Option<PathBuf>,
    admin_log_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    let server = NotionAdmin {
        inner: Inner::arc(
            client,
            AuditLog::new_with_admin(audit_log_path, admin_log_path),
        ),
    };
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
