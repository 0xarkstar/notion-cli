//! MCP stdio server — two surface variants, selected by the
//! `--allow-write` flag.

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content};
use rmcp::transport::stdio;
use rmcp::{tool, tool_router, ErrorData, ServiceExt};

use crate::api::NotionClient;
use crate::mcp::audit::AuditLog;
use crate::mcp::handlers;
use crate::mcp::params::{
    AppendBlockChildrenParams, CreateDataSourceParams, CreatePageParams, DeleteBlockParams,
    GetBlockParams, GetDataSourceParams, GetPageParams, ListBlockChildrenParams,
    QueryDataSourceParams, SearchParams, UpdateBlockParams, UpdatePageParams,
};

struct Inner {
    client: NotionClient,
    audit: AuditLog,
}

// === Read-only surface ====================================================

#[derive(Clone)]
pub struct NotionReadOnly {
    inner: Arc<Inner>,
}

#[tool_router(server_handler)]
impl NotionReadOnly {
    #[tool(
        name = "get_page",
        description = "Retrieve a Notion page by ID. Returns the page object wrapped in an untrusted-source envelope."
    )]
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
        description = "Query pages inside a data source with optional filter, sort, and pagination. Returns a paginated list of pages."
    )]
    async fn query_data_source(
        &self,
        params: Parameters<QueryDataSourceParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(to_result(&handlers::query_data_source(&self.inner.client, params.0).await?))
    }

    #[tool(
        name = "search",
        description = "Full-text search across Notion pages and data sources the integration can access."
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
        description = "List child blocks of a parent block (a page ID is also a block ID). Returns paginated results."
    )]
    async fn list_block_children(
        &self,
        params: Parameters<ListBlockChildrenParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(to_result(
            &handlers::list_block_children(&self.inner.client, params.0).await?,
        ))
    }
}

// === Full surface (read + write) =========================================

#[derive(Clone)]
pub struct NotionFull {
    inner: Arc<Inner>,
}

#[tool_router(server_handler)]
impl NotionFull {
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
        name = "append_block_children",
        description = "Append new child blocks to a parent block. Children are block bodies (e.g. paragraph, heading_1, to_do). Write operation — audited."
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
}

// === Entry points =========================================================

pub async fn run_read_only(client: NotionClient) -> anyhow::Result<()> {
    let server = NotionReadOnly {
        inner: Arc::new(Inner {
            client,
            audit: AuditLog::default(),
        }),
    };
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

pub async fn run_with_write(
    client: NotionClient,
    audit_log_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    let server = NotionFull {
        inner: Arc::new(Inner {
            client,
            audit: AuditLog::new(audit_log_path),
        }),
    };
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

fn to_result(value: &serde_json::Value) -> CallToolResult {
    let text = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
    CallToolResult::success(vec![Content::text(text)])
}
