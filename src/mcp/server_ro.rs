//! Read-only MCP tier — 7 query/read tools, no mutations.
//!
//! Default tier. Exposed without any flags. Safe for any agent
//! runtime — no write path, no schema mutation, no admin ops.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::transport::stdio;
use rmcp::{tool, tool_router, ErrorData, ServiceExt};

use crate::api::NotionClient;
use crate::mcp::audit::AuditLog;
use crate::mcp::common::{to_result, Inner};
use crate::mcp::handlers;
use crate::mcp::params::{
    GetBlockParams, GetDataSourceParams, GetPageParams, ListBlockChildrenParams,
    QueryDataSourceParams, SearchParams, UsersMeParams,
};

#[derive(Clone)]
pub struct NotionReadOnly {
    inner: std::sync::Arc<Inner>,
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
}

pub async fn run_read_only(client: NotionClient) -> anyhow::Result<()> {
    let server = NotionReadOnly {
        inner: Inner::arc(client, AuditLog::default()),
    };
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
