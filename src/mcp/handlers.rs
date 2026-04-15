//! Shared handler logic — called by both the read-only and full
//! server surfaces.

use std::collections::HashMap;

use rmcp::ErrorData;

use crate::api::data_source::{
    CreateDataSourceParent, CreateDataSourceRequest, QueryDataSourceRequest,
};
use crate::api::page::{CreatePageRequest, PageParent, UpdatePageRequest};
use crate::api::search::SearchRequest;
use crate::api::{ApiError, NotionClient};
use crate::mcp::params::{
    CreateDataSourceParams, CreatePageParams, GetDataSourceParams, GetPageParams,
    QueryDataSourceParams, SearchParams, UpdatePageParams,
};
use crate::output::wrap_untrusted;
use crate::types::property::PropertyValue;
use crate::types::rich_text::{Annotations, RichText, RichTextContent, TextContent};
use crate::validation::{DataSourceId, DatabaseId, PageId};

fn api_to_rpc(e: &ApiError) -> ErrorData {
    match e {
        ApiError::Unauthorized | ApiError::NotFound | ApiError::Validation { .. } => {
            ErrorData::invalid_params(e.to_string(), None)
        }
        ApiError::RateLimited { .. }
        | ApiError::ServerError { .. }
        | ApiError::Network { .. }
        | ApiError::BodyTooLarge { .. }
        | ApiError::Json(_) => ErrorData::internal_error(e.to_string(), None),
    }
}

fn validate<T, E: std::fmt::Display>(
    r: Result<T, E>,
    field: &str,
) -> Result<T, ErrorData> {
    r.map_err(|e| ErrorData::invalid_params(format!("{field}: {e}"), None))
}

fn parse_json<T: serde::de::DeserializeOwned>(
    v: &serde_json::Value,
    field: &str,
) -> Result<T, ErrorData> {
    serde_json::from_value(v.clone())
        .map_err(|e| ErrorData::invalid_params(format!("{field}: {e}"), None))
}

pub async fn get_page(
    client: &NotionClient,
    p: GetPageParams,
) -> Result<serde_json::Value, ErrorData> {
    let id = validate(PageId::from_url_or_id(&p.page_id), "page_id")?;
    let page = client.retrieve_page(&id).await.map_err(|e| api_to_rpc(&e))?;
    Ok(wrap_untrusted(&serde_json::to_value(page).map_err(|e| {
        ErrorData::internal_error(format!("serialize: {e}"), None)
    })?))
}

pub async fn get_data_source(
    client: &NotionClient,
    p: GetDataSourceParams,
) -> Result<serde_json::Value, ErrorData> {
    let id = validate(
        DataSourceId::from_url_or_id(&p.data_source_id),
        "data_source_id",
    )?;
    let ds = client.retrieve_data_source(&id).await.map_err(|e| api_to_rpc(&e))?;
    Ok(wrap_untrusted(&serde_json::to_value(ds).map_err(|e| {
        ErrorData::internal_error(format!("serialize: {e}"), None)
    })?))
}

pub async fn query_data_source(
    client: &NotionClient,
    p: QueryDataSourceParams,
) -> Result<serde_json::Value, ErrorData> {
    let id = validate(
        DataSourceId::from_url_or_id(&p.data_source_id),
        "data_source_id",
    )?;
    let sorts: Vec<_> = p
        .sorts
        .as_ref()
        .map(|v| parse_json(v, "sorts"))
        .transpose()?
        .unwrap_or_default();
    let req = QueryDataSourceRequest {
        filter: p.filter,
        sorts,
        start_cursor: p.start_cursor,
        page_size: p.page_size,
    };
    let resp = client.query_data_source(&id, &req).await.map_err(|e| api_to_rpc(&e))?;
    Ok(wrap_untrusted(&serde_json::to_value(resp).map_err(|e| {
        ErrorData::internal_error(format!("serialize: {e}"), None)
    })?))
}

pub async fn search(
    client: &NotionClient,
    p: SearchParams,
) -> Result<serde_json::Value, ErrorData> {
    let req = SearchRequest {
        query: p.query,
        filter: p.filter,
        sort: p.sort,
        start_cursor: p.start_cursor,
        page_size: p.page_size,
    };
    let resp = client.search(&req).await.map_err(|e| api_to_rpc(&e))?;
    Ok(wrap_untrusted(&serde_json::to_value(resp).map_err(|e| {
        ErrorData::internal_error(format!("serialize: {e}"), None)
    })?))
}

pub async fn create_page(
    client: &NotionClient,
    p: CreatePageParams,
) -> Result<serde_json::Value, ErrorData> {
    let parent = match (&p.parent_data_source_id, &p.parent_page_id) {
        (Some(ds), None) => PageParent::DataSource {
            data_source_id: validate(
                DataSourceId::from_url_or_id(ds),
                "parent_data_source_id",
            )?,
        },
        (None, Some(pg)) => PageParent::Page {
            page_id: validate(PageId::from_url_or_id(pg), "parent_page_id")?,
        },
        _ => {
            return Err(ErrorData::invalid_params(
                "exactly one of parent_data_source_id or parent_page_id required",
                None,
            ));
        }
    };
    let properties: HashMap<String, PropertyValue> = parse_json(&p.properties, "properties")?;
    let req = CreatePageRequest { parent, properties };
    let page = client.create_page(&req).await.map_err(|e| api_to_rpc(&e))?;
    Ok(wrap_untrusted(&serde_json::to_value(page).map_err(|e| {
        ErrorData::internal_error(format!("serialize: {e}"), None)
    })?))
}

pub async fn update_page(
    client: &NotionClient,
    p: UpdatePageParams,
) -> Result<serde_json::Value, ErrorData> {
    let id = validate(PageId::from_url_or_id(&p.page_id), "page_id")?;
    let properties: HashMap<String, PropertyValue> = p
        .properties
        .as_ref()
        .map(|v| parse_json(v, "properties"))
        .transpose()?
        .unwrap_or_default();
    let req = UpdatePageRequest {
        properties,
        archived: p.archived,
        in_trash: p.in_trash,
    };
    let page = client.update_page(&id, &req).await.map_err(|e| api_to_rpc(&e))?;
    Ok(wrap_untrusted(&serde_json::to_value(page).map_err(|e| {
        ErrorData::internal_error(format!("serialize: {e}"), None)
    })?))
}

pub async fn create_data_source(
    client: &NotionClient,
    p: CreateDataSourceParams,
) -> Result<serde_json::Value, ErrorData> {
    let db_id = validate(
        DatabaseId::from_url_or_id(&p.parent_database_id),
        "parent_database_id",
    )?;
    let title_vec = p
        .title
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(plain_title)
        .unwrap_or_default();
    let req = CreateDataSourceRequest {
        parent: CreateDataSourceParent::database(db_id),
        title: title_vec,
        properties: p.properties,
    };
    let ds = client.create_data_source(&req).await.map_err(|e| api_to_rpc(&e))?;
    Ok(wrap_untrusted(&serde_json::to_value(ds).map_err(|e| {
        ErrorData::internal_error(format!("serialize: {e}"), None)
    })?))
}

fn plain_title(s: &str) -> Vec<RichText> {
    vec![RichText {
        content: RichTextContent::Text {
            text: TextContent { content: s.to_string(), link: None },
        },
        annotations: Annotations::default(),
        plain_text: s.to_string(),
        href: None,
    }]
}
