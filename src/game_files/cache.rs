//! Game files kept on disk once fetched, each beside a zstd copy.
//!
//! Locations on the resources CDN are content-addressed, so a file never
//! changes and is fetched from CCP once. The layout (`<root>/<host>/<location>`)
//! is the one the render worker's studio (`frontend/scripts/render/studio.ts`)
//! writes, so the two share a cache. A file that zstd does not shrink by enough gets an empty marker
//! instead, so it is not compressed again on every request.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use axum::body::Bytes;

use crate::cdn::{CdnClient, FetchError, Host};

/// zstd's level for the stored copies: fast enough to compress on the first
/// request (a few hundred MB/s), and most of the ratio of the slow levels.
const ZSTD_LEVEL: i32 = 3;
/// A zstd copy is kept only below this share of the original, in percent:
/// textures and audio that are compressed already barely shrink.
const WORTH_PERCENT: usize = 95;
/// Beside a file, its zstd copy and the marker for "not worth compressing".
const ZSTD_SUFFIX: &str = ".zst";
const PLAIN_SUFFIX: &str = ".plain";

/// Distinguishes the temporary files of concurrent writes.
static WRITES: AtomicU64 = AtomicU64::new(0);

/// How a file's bytes are encoded on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// As stored upstream.
    Identity,
    /// The zstd copy (`Content-Encoding: zstd`).
    Zstd,
}

/// A file as served: its bytes and their encoding.
#[derive(Debug)]
pub struct Served {
    pub bytes: Bytes,
    pub encoding: Encoding,
}

#[derive(Clone, Debug)]
pub struct FileCache {
    root: PathBuf,
}

impl FileCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// A resources CDN file, from disk or fetched (and stored) once; the zstd
    /// copy when the client `accepts` it and one is worth having.
    ///
    /// # Errors
    ///
    /// The [`FetchError`] of a file that is neither on disk nor upstream.
    pub async fn resource(
        &self,
        cdn: &CdnClient,
        location: &str,
        accepts: Encoding,
    ) -> Result<Served, FetchError> {
        if !crate::cdn::is_safe_path(location) {
            return Err(FetchError::Missing);
        }
        let raw = self
            .root
            .join(cdn.host_name(Host::Resources))
            .join(location);
        let packed = with_suffix(&raw, ZSTD_SUFFIX);
        let plain = with_suffix(&raw, PLAIN_SUFFIX);

        let zstd = accepts == Encoding::Zstd;
        if zstd && let Ok(bytes) = tokio::fs::read(&packed).await {
            return Ok(Served {
                bytes: bytes.into(),
                encoding: Encoding::Zstd,
            });
        }
        let bytes: Bytes = match tokio::fs::read(&raw).await {
            Ok(bytes) => bytes.into(),
            Err(_) => {
                let bytes = cdn.fetch(Host::Resources, location).await?;
                write(&raw, &bytes).await;
                bytes
            }
        };
        if !zstd || tokio::fs::try_exists(&plain).await.unwrap_or(false) {
            return Ok(Served {
                bytes,
                encoding: Encoding::Identity,
            });
        }
        match compress(bytes.clone()).await {
            Some(compressed) => {
                write(&packed, &compressed).await;
                Ok(Served {
                    bytes: compressed,
                    encoding: Encoding::Zstd,
                })
            }
            None => {
                write(&plain, &[]).await;
                Ok(Served {
                    bytes,
                    encoding: Encoding::Identity,
                })
            }
        }
    }
}

/// The zstd copy of `bytes`, or `None` when it would not save enough.
async fn compress(bytes: Bytes) -> Option<Bytes> {
    let compressed = tokio::task::spawn_blocking(move || {
        zstd::bulk::compress(&bytes, ZSTD_LEVEL)
            .ok()
            .filter(|compressed| compressed.len() * 100 < bytes.len() * WORTH_PERCENT)
    })
    .await
    .ok()??;
    Some(compressed.into())
}

/// `path` with `suffix` added to its file name.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    name.into()
}

/// Writes a file aside and moves it into place, so a reader never sees half
/// of one. A failure only costs the cache, so it is logged and skipped.
async fn write(path: &Path, bytes: &[u8]) {
    let part = with_suffix(
        path,
        &format!(
            ".part-{}-{}",
            std::process::id(),
            WRITES.fetch_add(1, Ordering::Relaxed)
        ),
    );
    let result = async {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&part, bytes).await?;
        tokio::fs::rename(&part, path).await
    }
    .await;
    if let Err(error) = result {
        tracing::warn!(%error, path = %path.display(), "game files: could not cache");
        let _ = tokio::fs::remove_file(&part).await;
    }
}
