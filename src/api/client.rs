//! HTTP transport for the Notion REST API.
//!
//! Responsibilities:
//! - Auth header (`Authorization: Bearer …`) — sensitive-marked so
//!   reqwest's debug output redacts it.
//! - API version pin (`Notion-Version: 2026-03-11`) — see
//!   [`super::version`].
//! - Rate limiting: 3 req/s via [`governor`].
//! - Response body cap: 10 MiB via streaming consumption — oversized
//!   payloads fail with [`ApiError::BodyTooLarge`] rather than
//!   OOM-ing the process.
//! - Retry: on HTTP 429 only, honouring `Retry-After`, capped at 3
//!   attempts. 5xx errors propagate to the caller for application-
//!   level retry.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, RETRY_AFTER};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::api::error::ApiError;
use crate::api::version::{NOTION_API_BASE, NOTION_API_VERSION};
use crate::config::NotionToken;

pub const MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;
const MAX_RETRY_ATTEMPTS: u32 = 3;
const DEFAULT_RATE_LIMIT_PER_SEC: u32 = 3;

type DirectRateLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

pub struct NotionClient {
    http: reqwest::Client,
    base_url: String,
    rate_limiter: Arc<DirectRateLimiter>,
    max_response_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub base_url: String,
    pub connect_timeout: Duration,
    pub total_timeout: Duration,
    pub max_response_bytes: usize,
    pub rate_limit_per_sec: NonZeroU32,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            base_url: NOTION_API_BASE.into(),
            connect_timeout: Duration::from_secs(10),
            total_timeout: Duration::from_secs(60),
            max_response_bytes: MAX_RESPONSE_BYTES,
            rate_limit_per_sec: NonZeroU32::new(DEFAULT_RATE_LIMIT_PER_SEC)
                .expect("3 is non-zero"),
        }
    }
}

impl NotionClient {
    pub fn new(token: &NotionToken) -> Result<Self, ApiError> {
        Self::with_config(token, ClientConfig::default())
    }

    pub fn with_config(token: &NotionToken, config: ClientConfig) -> Result<Self, ApiError> {
        let http = reqwest::Client::builder()
            .connect_timeout(config.connect_timeout)
            .timeout(config.total_timeout)
            .user_agent(concat!("notion-cli/", env!("CARGO_PKG_VERSION")))
            .default_headers(build_default_headers(token)?)
            .build()
            .map_err(|e| ApiError::network("build", e.without_url().to_string()))?;

        let quota = Quota::per_second(config.rate_limit_per_sec);
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        Ok(Self {
            http,
            base_url: config.base_url,
            rate_limiter,
            max_response_bytes: config.max_response_bytes,
        })
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        self.request::<_, T>(Method::GET, path, None::<&()>).await
    }

    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        self.request(Method::POST, path, Some(body)).await
    }

    pub async fn patch<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        self.request(Method::PATCH, path, Some(body)).await
    }

    async fn request<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, ApiError> {
        for attempt in 1..=MAX_RETRY_ATTEMPTS {
            self.rate_limiter.until_ready().await;
            let result = self.do_request_once::<B, T>(method.clone(), path, body).await;
            match &result {
                Err(ApiError::RateLimited { retry_after }) if attempt < MAX_RETRY_ATTEMPTS => {
                    let wait = retry_after.unwrap_or(1).min(30);
                    tokio::time::sleep(Duration::from_secs(wait)).await;
                }
                _ => return result,
            }
        }
        // Unreachable because the final attempt above returns directly,
        // but keeps the type-checker happy.
        Err(ApiError::RateLimited { retry_after: None })
    }

    async fn do_request_once<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, ApiError> {
        let url = format!("{}/v1{}", self.base_url, path);
        let mut req = self.http.request(method, &url);
        if let Some(b) = body {
            req = req.json(b);
        }

        let resp = req.send().await.map_err(scrub_reqwest_err)?;
        let status = resp.status();

        if status == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = resp
                .headers()
                .get(RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());
            return Err(ApiError::RateLimited { retry_after });
        }

        let bytes = read_capped(resp, self.max_response_bytes).await?;

        if status.is_success() {
            serde_json::from_slice::<T>(&bytes).map_err(ApiError::Json)
        } else {
            Err(classify_error(status, &bytes))
        }
    }
}

fn build_default_headers(token: &NotionToken) -> Result<HeaderMap, ApiError> {
    let mut h = HeaderMap::new();

    let auth_raw = format!("Bearer {}", token.expose());
    let mut auth_val = HeaderValue::from_str(&auth_raw)
        .map_err(|e| ApiError::network("invalid_token", e.to_string()))?;
    auth_val.set_sensitive(true);
    h.insert(AUTHORIZATION, auth_val);

    h.insert(
        "Notion-Version",
        HeaderValue::from_static(NOTION_API_VERSION),
    );
    h.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    Ok(h)
}

pub(crate) fn scrub_reqwest_err(e: reqwest::Error) -> ApiError {
    let kind = if e.is_timeout() {
        "timeout"
    } else if e.is_connect() {
        "connect"
    } else if e.is_decode() {
        "decode"
    } else if e.is_body() {
        "body"
    } else {
        "other"
    };
    // `without_url` avoids leaking the request URL; Notion paths would
    // never contain tokens, but it's defense-in-depth.
    let stripped = e.without_url();
    ApiError::network(kind, stripped.to_string())
}

async fn read_capped(resp: reqwest::Response, limit: usize) -> Result<Vec<u8>, ApiError> {
    if let Some(len) = resp.content_length() {
        if len > limit as u64 {
            return Err(ApiError::BodyTooLarge { limit_bytes: limit });
        }
    }
    let mut buf = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(scrub_reqwest_err)?;
        if buf.len().saturating_add(chunk.len()) > limit {
            return Err(ApiError::BodyTooLarge { limit_bytes: limit });
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

#[derive(serde::Deserialize)]
struct NotionErrorBody {
    code: Option<String>,
    message: Option<String>,
}

fn classify_error(status: StatusCode, body: &[u8]) -> ApiError {
    let notion = serde_json::from_slice::<NotionErrorBody>(body).ok();
    let message = notion
        .as_ref()
        .and_then(|e| e.message.clone())
        .unwrap_or_else(|| String::from_utf8_lossy(body).to_string());
    let code = notion
        .as_ref()
        .and_then(|e| e.code.clone())
        .unwrap_or_else(|| format!("http_{}", status.as_u16()));

    match status.as_u16() {
        401 => ApiError::Unauthorized,
        404 => ApiError::NotFound,
        400 | 409 | 422 => ApiError::Validation { code, message },
        s if (500..600).contains(&s) => ApiError::ServerError { status: s, message },
        s => ApiError::Validation {
            code: format!("http_{s}"),
            message,
        },
    }
}
