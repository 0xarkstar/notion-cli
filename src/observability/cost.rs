//! API cost estimation for --check-request --cost.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CostEstimate {
    /// How many HTTP calls the full operation will make.
    pub api_calls: usize,
    /// Notion rate-limit window is 3 req/s average. Seconds required
    /// to clear `api_calls` at that ceiling.
    pub min_seconds: f64,
    /// Which endpoints will be hit (for operator visibility).
    pub endpoints: Vec<String>,
}

impl CostEstimate {
    #[must_use]
    pub fn single(endpoint: &str) -> Self {
        Self {
            api_calls: 1,
            min_seconds: 1.0 / 3.0,
            endpoints: vec![endpoint.to_string()],
        }
    }

    #[must_use]
    pub fn paginated(endpoint: &str, estimated_pages: usize) -> Self {
        #[allow(clippy::cast_precision_loss)]
        let min_seconds = estimated_pages as f64 / 3.0;
        Self {
            api_calls: estimated_pages,
            min_seconds,
            endpoints: vec![endpoint.to_string()],
        }
    }

    #[must_use]
    pub fn multi(endpoints: Vec<String>) -> Self {
        let n = endpoints.len();
        #[allow(clippy::cast_precision_loss)]
        let min_seconds = n as f64 / 3.0;
        Self {
            api_calls: n,
            min_seconds,
            endpoints,
        }
    }
}
