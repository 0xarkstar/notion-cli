//! Write MCP tier — 13 tools (7 RO + 6 write), opt-in via
//! `--allow-write`.
//!
//! Runtime CRUD surface: create/update pages, create data sources,
//! append/update/delete blocks. Every write is audited to the JSONL
//! sink configured by `NOTION_CLI_AUDIT_LOG`.
//!
//! Does NOT include admin lifecycle ops (db create, ds update,
//! relation wiring, page move) — those live in `server_admin.rs`
//! behind `--allow-admin`.

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
    AppendBlockChildrenParams, CreateDataSourceParams, CreatePageParams, DeleteBlockParams,
    GetBlockParams, GetDataSourceParams, GetPageParams, ListBlockChildrenParams,
    QueryDataSourceParams, SearchParams, UpdateBlockParams, UpdatePageParams, UsersMeParams,
};

#[derive(Clone)]
pub struct NotionWrite {
    inner: std::sync::Arc<Inner>,
}

#[tool_router(server_handler)]
impl NotionWrite {
    // --- reads (same as RO tier) ------------------------------------

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

    #[tool(
        name = "users_me",
        description = "Retrieve the bot user tied to the current integration token. Returns only the caller's own identity — does NOT enumerate workspace users."
    )]
    async fn users_me(
        &self,
        _params: Parameters<UsersMeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(to_result(&handlers::users_me(&self.inner.client).await?))
    }

    // --- writes -----------------------------------------------------

    #[tool(
        name = "create_page",
        description = "Create a Notion page under a data source or another page. Write operation — audited."
    )]
    async fn create_page(
        &self,
        params: Parameters<CreatePageParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let request_id = crate::observability::RequestId::new();
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
            Some(request_id.as_str()),
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
        let request_id = crate::observability::RequestId::new();
        let target = params.0.page_id.clone();
        let result = handlers::update_page(&self.inner.client, params.0).await;
        self.inner.audit.record(
            "update_page",
            Some(&target),
            result.as_ref().map(|_| ()).map_err(|e| e.message.as_ref()),
            Some(request_id.as_str()),
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
        let request_id = crate::observability::RequestId::new();
        let target = params.0.parent_database_id.clone();
        let result = handlers::create_data_source(&self.inner.client, params.0).await;
        self.inner.audit.record(
            "create_data_source",
            Some(&target),
            result.as_ref().map(|_| ()).map_err(|e| e.message.as_ref()),
            Some(request_id.as_str()),
        );
        Ok(to_result(&result?))
    }

    #[tool(
        name = "append_block_children",
        description = "Append new child blocks to a parent block. Children are block bodies (e.g. paragraph, heading_1, to_do). Write operation — audited."
    )]
    async fn append_block_children(
        &self,
        params: Parameters<AppendBlockChildrenParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let request_id = crate::observability::RequestId::new();
        let target = params.0.block_id.clone();
        let result = handlers::append_block_children(&self.inner.client, params.0).await;
        self.inner.audit.record(
            "append_block_children",
            Some(&target),
            result.as_ref().map(|_| ()).map_err(|e| e.message.as_ref()),
            Some(request_id.as_str()),
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
        let request_id = crate::observability::RequestId::new();
        let target = params.0.block_id.clone();
        let result = handlers::update_block(&self.inner.client, params.0).await;
        self.inner.audit.record(
            "update_block",
            Some(&target),
            result.as_ref().map(|_| ()).map_err(|e| e.message.as_ref()),
            Some(request_id.as_str()),
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
        let request_id = crate::observability::RequestId::new();
        let target = params.0.block_id.clone();
        let result = handlers::delete_block(&self.inner.client, params.0).await;
        self.inner.audit.record(
            "delete_block",
            Some(&target),
            result.as_ref().map(|_| ()).map_err(|e| e.message.as_ref()),
            Some(request_id.as_str()),
        );
        Ok(to_result(&result?))
    }
}

pub async fn run_with_write(
    client: NotionClient,
    audit_log_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    let server = NotionWrite {
        inner: Inner::arc(client, AuditLog::new(audit_log_path)),
    };
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
