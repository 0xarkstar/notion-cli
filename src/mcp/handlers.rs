//! Shared handler logic — called by both the read-only and full
//! server surfaces.

use std::collections::HashMap;

use rmcp::ErrorData;

use crate::api::block::{AppendBlockChildrenRequest, UpdateBlockRequest};
use crate::api::data_source::{
    CreateDataSourceParent, CreateDataSourceRequest, QueryDataSourceRequest,
    RelationDirection, SelectKind, UpdateDataSourceRequest,
};
use crate::api::database::{
    CreateDatabaseParent, CreateDatabaseRequest, InitialDataSource,
};
use crate::api::page::{CreatePageRequest, PageParent, UpdatePageRequest};
use crate::api::search::SearchRequest;
use crate::api::{ApiError, NotionClient};
use crate::mcp::params::{
    AppendBlockChildrenParams, CreateDataSourceParams, CreatePageParams, DbCreateParams,
    DeleteBlockParams, DsAddRelationParams, DsUpdateParams, GetBlockParams,
    GetDataSourceParams, GetPageParams, ListBlockChildrenParams, QueryDataSourceParams,
    SearchParams, UpdateBlockParams, UpdatePageParams,
};
use crate::output::wrap_untrusted;
use crate::types::block::BlockBody;
use crate::types::common::SelectOption;
use crate::types::icon::{Cover, Icon};
use crate::types::property::PropertyValue;
use crate::types::property_schema::PropertySchema;
use crate::types::rich_text::RichText;
use crate::validation::{BlockId, DataSourceId, DatabaseId, PageId};

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
    let children: Vec<BlockBody> = p
        .children
        .as_ref()
        .map(|v| parse_json(v, "children"))
        .transpose()?
        .unwrap_or_default();
    let icon = parse_icon_flag_mcp(p.icon.as_deref())?;
    let cover = parse_cover_flag_mcp(p.cover.as_deref())?;
    let req = CreatePageRequest {
        parent,
        properties,
        children,
        icon: icon.flatten(),
        cover: cover.flatten(),
    };
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
    let icon = parse_icon_flag_mcp(p.icon.as_deref())?;
    let cover = parse_cover_flag_mcp(p.cover.as_deref())?;
    let req = UpdatePageRequest {
        properties,
        archived: p.archived,
        in_trash: p.in_trash,
        icon,
        cover,
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
        .map(RichText::plain)
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

// === Admin handlers =======================================================

pub async fn ds_update(
    client: &NotionClient,
    p: DsUpdateParams,
) -> Result<serde_json::Value, ErrorData> {
    let id = validate(
        DataSourceId::from_url_or_id(&p.data_source_id),
        "data_source_id",
    )?;
    let req = build_ds_update(&p)?;
    let ds = client
        .update_data_source(&id, &req)
        .await
        .map_err(|e| api_to_rpc(&e))?;
    Ok(wrap_untrusted(&serde_json::to_value(ds).map_err(|e| {
        ErrorData::internal_error(format!("serialize: {e}"), None)
    })?))
}

fn build_ds_update(p: &DsUpdateParams) -> Result<UpdateDataSourceRequest, ErrorData> {
    match p.action.as_str() {
        "add_property" => {
            let name = field_str(p.name.as_deref(), "add_property", "name")?;
            let schema_json = p
                .schema
                .as_ref()
                .ok_or_else(|| invalid("add_property: 'schema' required"))?;
            let schema: PropertySchema = parse_json(schema_json, "schema")?;
            UpdateDataSourceRequest::add_property(name, &schema).map_err(|e| {
                ErrorData::invalid_params(format!("build add_property: {e}"), None)
            })
        }
        "remove_property" => {
            // D1 two-factor gate: param AND env
            if p.confirm != Some(true) {
                return Err(invalid(
                    "remove_property: destructive — pass confirm=true",
                ));
            }
            if std::env::var("NOTION_CLI_ADMIN_CONFIRMED").ok().as_deref() != Some("1") {
                return Err(invalid(
                    "remove_property: set NOTION_CLI_ADMIN_CONFIRMED=1 in the \
                     notion-cli mcp process environment to enable destructive ops",
                ));
            }
            let name = field_str(p.name.as_deref(), "remove_property", "name")?;
            Ok(UpdateDataSourceRequest::remove_property(name))
        }
        "rename_property" => {
            let old = field_str(p.name.as_deref(), "rename_property", "name")?;
            let new = field_str(p.new_name.as_deref(), "rename_property", "new_name")?;
            Ok(UpdateDataSourceRequest::rename_property(old, new))
        }
        "add_option" => {
            let prop = field_str(p.property.as_deref(), "add_option", "property")?;
            let kind_str = p.kind.as_deref().unwrap_or("select");
            let kind = SelectKind::parse(kind_str)
                .map_err(|e| invalid(format!("add_option: {e}")))?;
            let option_json = p
                .option
                .as_ref()
                .ok_or_else(|| invalid("add_option: 'option' required"))?;
            let option: SelectOption = parse_json(option_json, "option")?;
            Ok(UpdateDataSourceRequest::add_option(prop, kind, option))
        }
        "bulk" => {
            let body = p
                .body
                .as_ref()
                .ok_or_else(|| invalid("bulk: 'body' required"))?;
            UpdateDataSourceRequest::from_bulk(body.clone())
                .map_err(|e| invalid(format!("bulk: {e}")))
        }
        other => Err(invalid(format!(
            "unknown action '{other}' (expected: add_property, remove_property, \
             rename_property, add_option, bulk)"
        ))),
    }
}

fn field_str<'a>(
    v: Option<&'a str>,
    action: &str,
    field: &str,
) -> Result<&'a str, ErrorData> {
    v.ok_or_else(|| invalid(format!("{action}: '{field}' required")))
}

fn invalid(msg: impl Into<String>) -> ErrorData {
    ErrorData::invalid_params(msg.into(), None)
}

/// Parse MCP `icon` flag value.
///
/// - `None` → field absent (no change to page)
/// - `Some("none")` case-insensitive → `Some(None)` (clear)
/// - `Some(value)` → `Some(Some(Icon))` (parse_cli: http prefix → external, else emoji)
fn parse_icon_flag_mcp(value: Option<&str>) -> Result<Option<Option<Icon>>, ErrorData> {
    Ok(match value {
        None => None,
        Some(v) if v.eq_ignore_ascii_case("none") => Some(None),
        Some(v) => Some(Some(Icon::parse_cli(v))),
    })
}

/// Parse MCP `cover` flag value. Covers accept URLs only — `"none"`
/// clears, anything else must be a URL.
fn parse_cover_flag_mcp(value: Option<&str>) -> Result<Option<Option<Cover>>, ErrorData> {
    Ok(match value {
        None => None,
        Some(v) if v.eq_ignore_ascii_case("none") => Some(None),
        Some(v) if v.starts_with("http://") || v.starts_with("https://") => {
            Some(Some(Cover::external(v)))
        }
        Some(v) => {
            return Err(invalid(format!(
                "cover must be a URL (http:// or https://) or 'none' to clear, got: {v}"
            )));
        }
    })
}

pub async fn ds_add_relation(
    client: &NotionClient,
    p: DsAddRelationParams,
) -> Result<serde_json::Value, ErrorData> {
    let src_ds = validate(
        DataSourceId::from_url_or_id(&p.source_data_source_id),
        "source_data_source_id",
    )?;
    let self_ref = p.self_ref.unwrap_or(false);
    let one_way = p.one_way.unwrap_or(false);
    let has_backlink = p.backlink.is_some();
    let direction_count =
        usize::from(has_backlink) + usize::from(one_way) + usize::from(self_ref);
    if direction_count != 1 {
        return Err(invalid(
            "exactly one of backlink, one_way, self required",
        ));
    }
    let target_ds = if self_ref {
        if let Some(t) = p.target_data_source_id.as_deref() {
            let parsed = validate(DataSourceId::from_url_or_id(t), "target_data_source_id")?;
            if parsed.as_str() != src_ds.as_str() {
                return Err(invalid(
                    "self=true requires target to equal source (or omit target)",
                ));
            }
            parsed
        } else {
            src_ds.clone()
        }
    } else {
        let t = p
            .target_data_source_id
            .as_deref()
            .ok_or_else(|| invalid("target_data_source_id required unless self=true"))?;
        validate(DataSourceId::from_url_or_id(t), "target_data_source_id")?
    };
    let direction = if let Some(b) = p.backlink.as_deref() {
        RelationDirection::Dual(b.to_string())
    } else {
        RelationDirection::OneWay
    };
    // Pre-flight on target (skip when self_ref).
    if !self_ref {
        client
            .retrieve_data_source(&target_ds)
            .await
            .map_err(|e| api_to_rpc(&e))?;
    }
    let req = UpdateDataSourceRequest::add_relation_property(&p.name, target_ds, direction);
    let ds = client
        .update_data_source(&src_ds, &req)
        .await
        .map_err(|e| api_to_rpc(&e))?;
    Ok(wrap_untrusted(&serde_json::to_value(ds).map_err(|e| {
        ErrorData::internal_error(format!("serialize: {e}"), None)
    })?))
}

pub async fn db_create(
    client: &NotionClient,
    p: DbCreateParams,
) -> Result<serde_json::Value, ErrorData> {
    let parent_id = validate(PageId::from_url_or_id(&p.parent_page_id), "parent_page_id")?;
    let properties: HashMap<String, PropertySchema> =
        parse_json(&p.properties, "properties")?;
    let req = CreateDatabaseRequest {
        parent: CreateDatabaseParent::page(parent_id),
        title: RichText::plain(&p.title),
        initial_data_source: InitialDataSource { properties },
        icon: p.icon.as_deref().map(Icon::parse_cli),
        cover: p.cover.as_deref().map(Cover::external),
        is_inline: p.is_inline,
    };
    req.validate_local()
        .map_err(|e| ErrorData::invalid_params(e, None))?;
    let db = client.create_database(&req).await.map_err(|e| api_to_rpc(&e))?;
    Ok(wrap_untrusted(&serde_json::to_value(db).map_err(|e| {
        ErrorData::internal_error(format!("serialize: {e}"), None)
    })?))
}

// === Block handlers =======================================================

pub async fn get_block(
    client: &NotionClient,
    p: GetBlockParams,
) -> Result<serde_json::Value, ErrorData> {
    let id = validate(BlockId::from_url_or_id(&p.block_id), "block_id")?;
    let block = client.retrieve_block(&id).await.map_err(|e| api_to_rpc(&e))?;
    Ok(wrap_untrusted(&serde_json::to_value(block).map_err(|e| {
        ErrorData::internal_error(format!("serialize: {e}"), None)
    })?))
}

pub async fn list_block_children(
    client: &NotionClient,
    p: ListBlockChildrenParams,
) -> Result<serde_json::Value, ErrorData> {
    let id = validate(BlockId::from_url_or_id(&p.block_id), "block_id")?;
    let resp = client
        .list_block_children(&id, p.start_cursor.as_deref(), p.page_size)
        .await
        .map_err(|e| api_to_rpc(&e))?;
    Ok(wrap_untrusted(&serde_json::to_value(resp).map_err(|e| {
        ErrorData::internal_error(format!("serialize: {e}"), None)
    })?))
}

pub async fn append_block_children(
    client: &NotionClient,
    p: AppendBlockChildrenParams,
) -> Result<serde_json::Value, ErrorData> {
    let id = validate(BlockId::from_url_or_id(&p.block_id), "block_id")?;
    let children: Vec<BlockBody> = parse_json(&p.children, "children")?;
    let after = p
        .after
        .as_deref()
        .map(BlockId::from_url_or_id)
        .transpose()
        .map_err(|e| ErrorData::invalid_params(format!("after: {e}"), None))?;
    let req = AppendBlockChildrenRequest { children, after };
    let resp = client
        .append_block_children(&id, &req)
        .await
        .map_err(|e| api_to_rpc(&e))?;
    Ok(wrap_untrusted(&serde_json::to_value(resp).map_err(|e| {
        ErrorData::internal_error(format!("serialize: {e}"), None)
    })?))
}

pub async fn update_block(
    client: &NotionClient,
    p: UpdateBlockParams,
) -> Result<serde_json::Value, ErrorData> {
    let id = validate(BlockId::from_url_or_id(&p.block_id), "block_id")?;
    let body: Option<BlockBody> = p
        .body
        .as_ref()
        .map(|v| parse_json(v, "body"))
        .transpose()?;
    let req = UpdateBlockRequest {
        body,
        archived: p.archived,
        in_trash: p.in_trash,
    };
    let block = client.update_block(&id, &req).await.map_err(|e| api_to_rpc(&e))?;
    Ok(wrap_untrusted(&serde_json::to_value(block).map_err(|e| {
        ErrorData::internal_error(format!("serialize: {e}"), None)
    })?))
}

pub async fn delete_block(
    client: &NotionClient,
    p: DeleteBlockParams,
) -> Result<serde_json::Value, ErrorData> {
    let id = validate(BlockId::from_url_or_id(&p.block_id), "block_id")?;
    let block = client.delete_block(&id).await.map_err(|e| api_to_rpc(&e))?;
    Ok(wrap_untrusted(&serde_json::to_value(block).map_err(|e| {
        ErrorData::internal_error(format!("serialize: {e}"), None)
    })?))
}

