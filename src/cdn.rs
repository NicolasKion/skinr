//! Same-origin passthrough to the game client's CDNs.
//!
//! The browser runtime reads the client's file index and resources itself,
//! but CCP's CloudFront caches responses without keying on `Origin`, so a
//! browser often gets a copy without CORS headers. Serving them from our own
//! origin sidesteps CORS entirely.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

/// Where the client's build listings and file indexes live.
pub const DEFAULT_BINARIES_URL: &str = "https://binaries.eveonline.com";

/// Where the content-addressed files those indexes point at are served from.
pub const DEFAULT_RESOURCES_URL: &str = "https://resources.eveonline.com";

/// The build pointer (`eveclient_TQ.json`) moves with every patch; everything
/// else is named by build or content hash and never changes.
const MOVING_CACHE_CONTROL: &str = "public, max-age=60";
const IMMUTABLE_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";

#[derive(Clone)]
pub struct CdnClient {
    http: reqwest::Client,
    binaries_url: String,
    resources_url: String,
}

/// Which of the two hosts a request goes to.
#[derive(Clone, Copy)]
pub enum Host {
    Binaries,
    Resources,
}

impl CdnClient {
    pub fn new(binaries_url: &str, resources_url: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            binaries_url: binaries_url.trim_end_matches('/').to_owned(),
            resources_url: resources_url.trim_end_matches('/').to_owned(),
        }
    }

    /// The live hosts, overridable with `CDN_BINARIES_URL` / `CDN_RESOURCES_URL`
    /// (tests point them at local mocks).
    pub fn from_env() -> Self {
        let var =
            |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_owned());
        Self::new(
            &var("CDN_BINARIES_URL", DEFAULT_BINARIES_URL),
            &var("CDN_RESOURCES_URL", DEFAULT_RESOURCES_URL),
        )
    }

    fn url(&self, host: Host, path: &str) -> String {
        let base = match host {
            Host::Binaries => &self.binaries_url,
            Host::Resources => &self.resources_url,
        };
        format!("{base}/{path}")
    }

    /// Streams one upstream file back, with its content type and a cache
    /// lifetime matching how often it can change.
    pub async fn proxy(&self, host: Host, path: &str) -> Response {
        if !is_safe_path(path) {
            return StatusCode::NOT_FOUND.into_response();
        }

        let upstream = match self.http.get(self.url(host, path)).send().await {
            Ok(response) => response,
            Err(error) => {
                tracing::warn!(%error, path, "cdn: upstream request failed");
                return StatusCode::BAD_GATEWAY.into_response();
            }
        };

        match upstream.status() {
            status if status.is_success() => {}
            reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::FORBIDDEN => {
                // S3 answers a missing key with 403.
                return StatusCode::NOT_FOUND.into_response();
            }
            status => {
                tracing::warn!(%status, path, "cdn: upstream error");
                return StatusCode::BAD_GATEWAY.into_response();
            }
        }

        let content_type = upstream
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| HeaderValue::from_bytes(value.as_bytes()).ok())
            .unwrap_or(HeaderValue::from_static("application/octet-stream"));
        let cache_control = if is_moving(host, path) {
            MOVING_CACHE_CONTROL
        } else {
            IMMUTABLE_CACHE_CONTROL
        };

        // reqwest has already undone any gzip transfer, so no encoding or
        // length headers are carried over.
        (
            [
                (header::CONTENT_TYPE, content_type),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static(cache_control),
                ),
            ],
            Body::from_stream(upstream.bytes_stream()),
        )
            .into_response()
    }
}

/// Only plain relative paths: no traversal, queries or odd characters.
fn is_safe_path(path: &str) -> bool {
    !path.is_empty()
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '.'))
}

fn is_moving(host: Host, path: &str) -> bool {
    matches!(host, Host::Binaries) && path.starts_with("eveclient_")
}

pub async fn binaries(State(cdn): State<CdnClient>, Path(path): Path<String>) -> Response {
    cdn.proxy(Host::Binaries, &path).await
}

pub async fn resources(State(cdn): State<CdnClient>, Path(path): Path<String>) -> Response {
    cdn.proxy(Host::Resources, &path).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_paths() {
        assert!(is_safe_path("eveclient_TQ.json"));
        assert!(is_safe_path("4f/4f0c_9a1b2c"));
        assert!(!is_safe_path(""));
        assert!(!is_safe_path("../etc/passwd"));
        assert!(!is_safe_path("a//b"));
        assert!(!is_safe_path("a/./b"));
        assert!(!is_safe_path("a?b=c"));
        assert!(!is_safe_path("a%2e%2e"));
    }

    #[test]
    fn only_the_build_pointer_moves() {
        assert!(is_moving(Host::Binaries, "eveclient_TQ.json"));
        assert!(!is_moving(Host::Binaries, "eveonline_3599281.txt"));
        assert!(!is_moving(Host::Resources, "eveclient_TQ.json"));
    }
}
