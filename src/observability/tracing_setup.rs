//! Tracing subscriber bootstrap.

use tracing_subscriber::EnvFilter;

/// Install the default tracing subscriber. Safe to call multiple
/// times — subsequent calls after the first are no-ops.
pub fn init() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .try_init();
}
