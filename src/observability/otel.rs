//! OpenTelemetry exporter bootstrap (feature `otel` only).
//!
//! Enable with: `cargo build --features otel`.
//! Then run with `--otlp-endpoint <url>` (or env `OTEL_EXPORTER_OTLP_ENDPOINT`).
//! Falls back to the default gRPC endpoint when endpoint is empty.

// Placeholder — keep the crate compiling even if the specific OTel
// API surface changes. Minimal implementation:
pub fn install(_endpoint: Option<&str>) -> anyhow::Result<()> {
    // Skeleton only for v0.4 — real exporter install in v0.5.
    // opentelemetry-otlp::new_exporter().tonic().with_endpoint(endpoint) ...
    Ok(())
}
