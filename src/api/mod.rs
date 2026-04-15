//! Notion REST API client and typed endpoint wrappers.
//!
//! Layering:
//! - [`client::NotionClient`] — the HTTP transport with auth, rate
//!   limiting (3 req/s), response size cap (10 MiB), 429 retry.
//! - [`data_source`] — endpoints under `/v1/data_sources/*`. The
//!   `create_data_source` call here is the entire reason this crate
//!   exists (it is broken in `@notionhq/notion-mcp-server`).
//! - [`page`] — endpoints under `/v1/pages/*`.
//! - [`pagination`] — generic paginated response wrapper.
//!
//! All endpoints accept/return types from the [`crate::types`] module.
//! Write requests use `HashMap<String, PropertyValue>` directly (not
//! the [`crate::types::Property`] wrapper) so that `Raw` fallbacks
//! cannot accidentally leak into write payloads.

pub mod client;
pub mod data_source;
pub mod error;
pub mod page;
pub mod pagination;
pub mod version;

pub use client::{ClientConfig, NotionClient, MAX_RESPONSE_BYTES};
pub use error::ApiError;
pub use pagination::PaginatedResponse;
pub use version::{NOTION_API_BASE, NOTION_API_VERSION};
