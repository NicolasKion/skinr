//! A small client for ESI (https://esi.evetech.net), CCP's REST API.
//!
//! It follows ESI's rules for clients: every request names the app and pins a
//! compatibility date; a `429` waits out `Retry-After`, a `420` (the error
//! limit) waits for its reset, and the client slows down before a rate-limit
//! bucket runs dry rather than at it.

use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use reqwest::{RequestBuilder, Response, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Where ESI lives; tests point `ESI_URL` at a local mock.
const DEFAULT_ESI_URL: &str = "https://esi.evetech.net";
/// The API behavior this client was written against. Without it ESI answers
/// as of 2020-01-01, where the SKINR routes do not exist.
pub const COMPATIBILITY_DATE: &str = "2026-08-18";
/// Sent when `ESI_USER_AGENT` is unset; set it to add a contact address.
const DEFAULT_USER_AGENT: &str = "skinr.space/0.1";
/// Below this many tokens left in a route's bucket, pause before going on.
const LOW_TOKENS: i64 = 20;
/// How long to pause when a bucket runs low (tokens come back as the window slides).
const LOW_TOKENS_PAUSE: Duration = Duration::from_secs(60);
/// How often a request is retried after a 420 or 429 before giving up.
const MAX_RETRIES: u32 = 3;
/// Waited when a 420 or 429 does not say how long to wait.
const DEFAULT_BACKOFF: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub enum Error {
    Http(reqwest::Error),
    /// ESI answered with an error status (after any retries).
    Status(StatusCode, String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Http(error) => write!(f, "ESI request failed: {error}"),
            Error::Status(status, body) => write!(f, "ESI answered {status}: {body}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Error::Http(error)
    }
}

#[derive(Clone)]
pub struct EsiClient {
    http: reqwest::Client,
    base_url: String,
}

impl EsiClient {
    pub fn new(base_url: impl Into<String>, user_agent: &str) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Compatibility-Date",
            HeaderValue::from_static(COMPATIBILITY_DATE),
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(user_agent)
                .unwrap_or_else(|_| HeaderValue::from_static(DEFAULT_USER_AGENT)),
        );
        Self {
            http: reqwest::Client::builder()
                .default_headers(headers)
                .gzip(true)
                .build()
                .expect("an HTTP client"),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
        }
    }

    /// The client `ESI_URL` and `ESI_USER_AGENT` describe, with the defaults.
    pub fn from_env() -> Self {
        Self::new(
            std::env::var("ESI_URL").unwrap_or_else(|_| DEFAULT_ESI_URL.to_owned()),
            &std::env::var("ESI_USER_AGENT").unwrap_or_else(|_| DEFAULT_USER_AGENT.to_owned()),
        )
    }

    /// GETs `path` (with its query) and decodes the JSON body.
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let url = format!("{}{path}", self.base_url);
        let response = self.send(|| self.http.get(&url)).await?;
        Ok(response.json().await?)
    }

    /// POSTs `body` as JSON to `path` and decodes the JSON answer.
    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        let url = format!("{}{path}", self.base_url);
        let response = self.send(|| self.http.post(&url).json(body)).await?;
        Ok(response.json().await?)
    }

    async fn send(&self, request: impl Fn() -> RequestBuilder) -> Result<Response, Error> {
        let mut attempt = 0;
        loop {
            let response = request().send().await?;
            let status = response.status();
            let wait = match status.as_u16() {
                429 => Some(seconds_header(response.headers(), "Retry-After")),
                420 => Some(seconds_header(
                    response.headers(),
                    "X-ESI-Error-Limit-Reset",
                )),
                _ => None,
            };
            if let Some(wait) = wait {
                attempt += 1;
                if attempt > MAX_RETRIES {
                    return Err(Error::Status(
                        status,
                        response.text().await.unwrap_or_default(),
                    ));
                }
                let wait = wait.unwrap_or(DEFAULT_BACKOFF);
                tracing::warn!(%status, ?wait, "ESI asked us to back off");
                tokio::time::sleep(wait).await;
                continue;
            }
            if !status.is_success() {
                return Err(Error::Status(
                    status,
                    response.text().await.unwrap_or_default(),
                ));
            }
            let remaining = response
                .headers()
                .get("X-Ratelimit-Remaining")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<i64>().ok());
            if remaining.is_some_and(|remaining| remaining < LOW_TOKENS) {
                tracing::info!(?remaining, "ESI rate limit bucket is low, pausing");
                tokio::time::sleep(LOW_TOKENS_PAUSE).await;
            }
            return Ok(response);
        }
    }
}

fn seconds_header(headers: &HeaderMap, name: &str) -> Option<Duration> {
    headers
        .get(name)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}
