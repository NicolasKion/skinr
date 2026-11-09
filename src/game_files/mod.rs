//! Game files by their `res:/` path, so the browser needs no file index.
//!
//! The API reads the client's file index once per build (30 MB the browser
//! would otherwise download and parse on every visit) and serves:
//!
//! - `/cdn/build`: the current client build, `{"build": "…"}`;
//! - `/cdn/res/{build}/{path}`: one game file, kept on disk once fetched and
//!   sent zstd-compressed to browsers that accept it;
//! - `/cdn/list/{build}/{folder}`: the names of the files in one folder (the
//!   SOF asks whether a material exists before reading it).
//!
//! The build in the URL makes every answer immutable. Requests for an older
//! build keep working while it is one of the last few.

mod cache;
mod index;

use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::future::try_join_all;
use serde::Deserialize;

use crate::cdn::{CdnClient, FetchError, Host, IMMUTABLE_CACHE_CONTROL, MOVING_CACHE_CONTROL};
use cache::{Encoding, FileCache};
use index::ResIndex;

/// The client's build pointer on the binaries host.
const BUILD_POINTER: &str = "eveclient_TQ.json";
/// How often the build pointer is checked again, at most.
const BUILD_CHECK_INTERVAL: Duration = Duration::from_secs(60);
/// How many builds' indexes are kept, so pages open across a patch still load.
const KEPT_BUILDS: usize = 2;
/// Where fetched game files are kept by default (shared with the thumbnail script).
const DEFAULT_CACHE_DIR: &str = "storage/cdn-cache";

/// The cache directory from `CDN_CACHE_DIR`, with the local default.
pub fn cache_dir() -> PathBuf {
    std::env::var("CDN_CACHE_DIR")
        .unwrap_or_else(|_| DEFAULT_CACHE_DIR.to_owned())
        .into()
}

#[derive(Clone, Debug)]
pub struct GameFiles {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    cdn: CdnClient,
    cache: FileCache,
    /// The newest last.
    indexes: RwLock<Vec<Arc<ResIndex>>>,
    /// When the build pointer was last checked; held while one check runs.
    checked: tokio::sync::Mutex<Option<Instant>>,
}

#[derive(Deserialize)]
struct BuildPointer {
    build: serde_json::Value,
}

impl GameFiles {
    pub fn new(cdn: CdnClient, cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            inner: Arc::new(Inner {
                cdn,
                cache: FileCache::new(cache_dir),
                indexes: RwLock::new(Vec::new()),
                checked: tokio::sync::Mutex::new(None),
            }),
        }
    }

    /// Loads the current build's index in the background, so the first
    /// visitor does not wait for it.
    pub fn warm(&self) {
        let files = self.clone();
        tokio::spawn(async move {
            if files.current().await.is_err() {
                tracing::warn!("game files: could not load the file index; will retry on request");
            }
        });
    }

    fn known(&self, build: &str) -> Option<Arc<ResIndex>> {
        let indexes = self.inner.indexes.read().expect("lock poisoned");
        indexes.iter().find(|index| index.build == build).cloned()
    }

    fn latest(&self) -> Option<Arc<ResIndex>> {
        let indexes = self.inner.indexes.read().expect("lock poisoned");
        indexes.last().cloned()
    }

    /// The current build's index, loading it when the build pointer moved.
    ///
    /// # Errors
    ///
    /// The [`FetchError`] of a build pointer or index that could not be read.
    async fn current(&self) -> Result<Arc<ResIndex>, FetchError> {
        let mut checked = self.inner.checked.lock().await;
        let latest = self.latest();
        if let (Some(index), Some(at)) = (&latest, *checked)
            && at.elapsed() < BUILD_CHECK_INTERVAL
        {
            return Ok(index.clone());
        }
        let build = self.fetch_build().await?;
        if let Some(index) = latest.filter(|index| index.build == build) {
            *checked = Some(Instant::now());
            return Ok(index);
        }
        let index = Arc::new(self.load(&build).await?);
        tracing::info!(build, files = index.len(), "game files: index loaded");
        {
            let mut indexes = self.inner.indexes.write().expect("lock poisoned");
            indexes.push(index.clone());
            let excess = indexes.len().saturating_sub(KEPT_BUILDS);
            indexes.drain(..excess);
        }
        *checked = Some(Instant::now());
        Ok(index)
    }

    /// A build's index if it is current or still kept, else `None`.
    async fn index_for(&self, build: &str) -> Result<Option<Arc<ResIndex>>, FetchError> {
        if let Some(index) = self.known(build) {
            return Ok(Some(index));
        }
        let current = self.current().await?;
        Ok((current.build == build).then_some(current))
    }

    async fn fetch_build(&self) -> Result<String, FetchError> {
        let bytes = self.inner.cdn.fetch(Host::Binaries, BUILD_POINTER).await?;
        let pointer: BuildPointer = serde_json::from_slice(&bytes).map_err(|error| {
            tracing::warn!(%error, "game files: unreadable build pointer");
            FetchError::Upstream
        })?;
        match pointer.build {
            serde_json::Value::String(build) => Ok(build),
            serde_json::Value::Number(build) => Ok(build.to_string()),
            _ => Err(FetchError::Upstream),
        }
    }

    /// Fetches a build's app index and every resfileindex it names.
    async fn load(&self, build: &str) -> Result<ResIndex, FetchError> {
        let cdn = &self.inner.cdn;
        let app = cdn
            .fetch(Host::Binaries, &format!("eveonline_{build}.txt"))
            .await?;
        let app = String::from_utf8_lossy(&app);
        let texts = try_join_all(
            index::res_index_locations(&app)
                .into_iter()
                .map(|location| cdn.fetch(Host::Binaries, location)),
        )
        .await?;
        let build = build.to_owned();
        tokio::task::spawn_blocking(move || {
            let texts: Vec<String> = texts
                .iter()
                .map(|text| String::from_utf8_lossy(text).into_owned())
                .collect();
            ResIndex::from_texts(&build, texts.iter().map(String::as_str))
        })
        .await
        .map_err(|_| FetchError::Upstream)
    }
}

/// The best encoding the client takes: zstd when `Accept-Encoding` lists it
/// (and not with `q=0`).
fn accepted(headers: &HeaderMap) -> Encoding {
    let zstd = headers
        .get_all(header::ACCEPT_ENCODING)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|coding| {
            let mut parts = coding.split(';').map(str::trim);
            parts
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case("zstd"))
                && parts.all(|param| param.replace(' ', "") != "q=0")
        });
    if zstd {
        Encoding::Zstd
    } else {
        Encoding::Identity
    }
}

fn failed(error: FetchError) -> Response {
    match error {
        FetchError::Missing => StatusCode::NOT_FOUND.into_response(),
        FetchError::Upstream => StatusCode::BAD_GATEWAY.into_response(),
    }
}

/// `/cdn/build`.
pub async fn build(State(files): State<GameFiles>) -> Response {
    match files.current().await {
        Ok(index) => (
            [(
                header::CACHE_CONTROL,
                HeaderValue::from_static(MOVING_CACHE_CONTROL),
            )],
            Json(serde_json::json!({ "build": index.build })),
        )
            .into_response(),
        Err(error) => failed(error),
    }
}

/// `/cdn/res/{build}/{path}`.
pub async fn file(
    State(files): State<GameFiles>,
    Path((build, path)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let Some(path) = index::normalize(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let index = match files.index_for(&build).await {
        Ok(Some(index)) => index,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) => return failed(error),
    };
    let Some(location) = index.locate(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let served = match files
        .inner
        .cache
        .resource(&files.inner.cdn, location, accepted(&headers))
        .await
    {
        Ok(served) => served,
        Err(error) => return failed(error),
    };
    let mut response = (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static(IMMUTABLE_CACHE_CONTROL),
            ),
            (header::VARY, HeaderValue::from_static("accept-encoding")),
        ],
        served.bytes,
    )
        .into_response();
    if served.encoding == Encoding::Zstd {
        response
            .headers_mut()
            .insert(header::CONTENT_ENCODING, HeaderValue::from_static("zstd"));
    }
    response
}

/// `/cdn/list/{build}/{folder}`.
pub async fn list(
    State(files): State<GameFiles>,
    Path((build, folder)): Path<(String, String)>,
) -> Response {
    let Some(folder) = index::normalize(&folder) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match files.index_for(&build).await {
        Ok(Some(index)) => (
            [(
                header::CACHE_CONTROL,
                HeaderValue::from_static(IMMUTABLE_CACHE_CONTROL),
            )],
            Json(index.list(&folder)),
        )
            .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => failed(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCEPT_ENCODING,
            HeaderValue::from_str(value).unwrap(),
        );
        headers
    }

    #[test]
    fn reads_accept_encoding() {
        assert_eq!(
            accepted(&headers("gzip, deflate, br, zstd")),
            Encoding::Zstd
        );
        assert_eq!(accepted(&headers("zstd;q=0.5")), Encoding::Zstd);
        assert_eq!(accepted(&headers("gzip, br")), Encoding::Identity);
        assert_eq!(accepted(&headers("zstd;q=0")), Encoding::Identity);
        assert_eq!(accepted(&HeaderMap::new()), Encoding::Identity);
    }
}
