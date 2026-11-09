//! Same-origin passthrough to the game client's CDNs.
//!
//! The browser runtime reads the client's file index and resources itself,
//! but CCP's CloudFront caches responses without keying on `Origin`, so a
//! browser often gets a copy without CORS headers. Serving them from our own
//! origin sidesteps CORS entirely.

use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

/// Where the client's build listings and file indexes live.
pub const DEFAULT_BINARIES_URL: &str = "https://binaries.eveonline.com";

/// Where the content-addressed files those indexes point at are served from.
pub const DEFAULT_RESOURCES_URL: &str = "https://resources.eveonline.com";

/// The build pointer (`eveclient_TQ.json`) moves with every patch; everything
/// else is named by build or content hash and never changes.
pub const MOVING_CACHE_CONTROL: &str = "public, max-age=60";
pub const IMMUTABLE_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";

#[derive(Clone, Debug)]
pub struct CdnClient {
    http: reqwest::Client,
    binaries_url: String,
    resources_url: String,
}

/// Which of the two hosts a request goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    Binaries,
    Resources,
}

/// Why an upstream file could not be fetched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FetchError {
    /// The host does not have it (S3 answers a missing key with 403).
    Missing,
    /// The host could not be reached or answered with an error.
    Upstream,
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Missing => "not on the cdn",
            Self::Upstream => "cdn unreachable or failing",
        })
    }
}

impl std::error::Error for FetchError {}

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

    fn base(&self, host: Host) -> &str {
        match host {
            Host::Binaries => &self.binaries_url,
            Host::Resources => &self.resources_url,
        }
    }

    fn url(&self, host: Host, path: &str) -> String {
        format!("{}/{path}", self.base(host))
    }

    /// The host's name (`resources.eveonline.com`), which names its folder
    /// in a disk cache.
    pub fn host_name(&self, host: Host) -> &str {
        let base = self.base(host);
        let base = base.split_once("://").map_or(base, |(_, rest)| rest);
        base.split('/').next().unwrap_or(base)
    }

    /// Sends the request for one upstream file and checks its status.
    async fn request(&self, host: Host, path: &str) -> Result<reqwest::Response, FetchError> {
        if !is_safe_path(path) {
            return Err(FetchError::Missing);
        }
        let upstream = self
            .http
            .get(self.url(host, path))
            .send()
            .await
            .map_err(|error| {
                tracing::warn!(%error, path, "cdn: upstream request failed");
                FetchError::Upstream
            })?;
        match upstream.status() {
            status if status.is_success() => Ok(upstream),
            reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::FORBIDDEN => {
                Err(FetchError::Missing)
            }
            status => {
                tracing::warn!(%status, path, "cdn: upstream error");
                Err(FetchError::Upstream)
            }
        }
    }

    /// One upstream file, whole.
    ///
    /// # Errors
    ///
    /// [`FetchError::Missing`] for an unsafe path or one the host does not
    /// have, [`FetchError::Upstream`] when the host fails.
    pub async fn fetch(&self, host: Host, path: &str) -> Result<Bytes, FetchError> {
        self.request(host, path)
            .await?
            .bytes()
            .await
            .map_err(|error| {
                tracing::warn!(%error, path, "cdn: upstream body failed");
                FetchError::Upstream
            })
    }

    /// Streams one upstream file back, with its content type and a cache
    /// lifetime matching how often it can change.
    pub async fn proxy(&self, host: Host, path: &str) -> Response {
        let upstream = match self.request(host, path).await {
            Ok(upstream) => upstream,
            Err(FetchError::Missing) => return StatusCode::NOT_FOUND.into_response(),
            Err(FetchError::Upstream) => return StatusCode::BAD_GATEWAY.into_response(),
        };

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
pub fn is_safe_path(path: &str) -> bool {
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
    fn names_the_host() {
        let cdn = CdnClient::new("https://binaries.eveonline.com/", "http://127.0.0.1:8080");
        assert_eq!(cdn.host_name(Host::Binaries), "binaries.eveonline.com");
        assert_eq!(cdn.host_name(Host::Resources), "127.0.0.1:8080");
    }

    #[test]
    fn only_the_build_pointer_moves() {
        assert!(is_moving(Host::Binaries, "eveclient_TQ.json"));
        assert!(!is_moving(Host::Binaries, "eveonline_3599281.txt"));
        assert!(!is_moving(Host::Resources, "eveclient_TQ.json"));
    }
}
