//! Observability baseline — request correlation + tracing.
//!
//! # Request IDs
//!
//! Every CLI invocation and every MCP tool call gets a UUID v7
//! generated at entry. UUID v7 is time-sortable (millisecond-prefixed)
//! so audit logs and OpenTelemetry traces sort naturally by wall-clock order.
//! Time-leak note: the ID leaks creation time to whoever reads the
//! log. Audit JSONL is operator-local in this binary (no cross-tenant
//! sharing) so it's not a v0.4 concern — revisit if multi-tenant
//! gateway lands.
//!
//! # Tracing
//!
//! `tracing` spans wrap HTTP requests and MCP tool bodies. The default
//! subscriber writes structured log events to stderr (configurable
//! via `RUST_LOG`). The `otel` feature enables OTLP export on top.

pub mod cost;
pub mod request_id;
pub mod tracing_setup;

#[cfg(feature = "otel")]
pub mod otel;

pub use request_id::RequestId;
