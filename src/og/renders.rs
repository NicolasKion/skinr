//! Ships in a livery, drawn by the render worker
//! (`frontend/scripts/render-server.ts`) and kept on disk.
//!
//! A card waits a few seconds for its render. A slower one carries on in the
//! background and lands on disk for the next request, so the first unfurl
//! gets a fallback card and the ones after it the real thing.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use futures_util::FutureExt as _;
use futures_util::future::{BoxFuture, Shared};
use sha2::{Digest, Sha256};

/// Bump to draw every render again (a change to the worker's framing or lighting).
const RENDER_VERSION: u32 = 1;
/// The size renders are drawn at, in px: the size the card shows a single ship at.
pub const RENDER_SIZE: u32 = 580;
/// How long one render may take in the background (the worker queues them,
/// and lavapipe draws on the CPU).
const RENDER_TIMEOUT: Duration = Duration::from_secs(300);
/// A render that failed is not asked for again for this long.
const RETRY_AFTER: Duration = Duration::from_secs(600);
/// The first bytes of every PNG.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// What to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    /// A hull in a livery, by SOF DNA (an in-game skin).
    Dna(String),
    /// A SKINR design on its ship. `look` is everything the render depends
    /// on besides the id (the ship's DNA and the design's layout), so a
    /// change draws it again.
    Skinr {
        design_id: String,
        ship_id: i32,
        look: String,
    },
}

impl Job {
    /// The file name the render is kept under.
    pub fn key(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(format!("v{RENDER_VERSION}:{RENDER_SIZE}:"));
        match self {
            Self::Dna(dna) => hash.update(format!("dna:{dna}")),
            Self::Skinr {
                design_id,
                ship_id,
                look,
            } => hash.update(format!("skinr:{design_id}:{ship_id}:{look}")),
        }
        hex(&hash.finalize()[..16])
    }

    fn url(&self, base: &str) -> Option<reqwest::Url> {
        let size = RENDER_SIZE.to_string();
        match self {
            Self::Dna(dna) => reqwest::Url::parse_with_params(
                &format!("{base}/render"),
                [("dna", dna.as_str()), ("size", &size), ("theme", "dark")],
            ),
            Self::Skinr {
                design_id, ship_id, ..
            } => reqwest::Url::parse_with_params(
                &format!("{base}/render/skinr/{design_id}"),
                [
                    ("ship", ship_id.to_string().as_str()),
                    ("size", &size),
                    ("theme", "dark"),
                ],
            ),
        }
        .ok()
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

type Pending = Shared<BoxFuture<'static, Option<Bytes>>>;

/// The render worker and the renders it has drawn.
#[derive(Clone)]
pub struct Renders {
    inner: Arc<Inner>,
}

struct Inner {
    /// Where the worker listens; `None` turns renders off.
    base: Option<String>,
    dir: PathBuf,
    http: reqwest::Client,
    /// Renders being drawn, by key, so concurrent cards share one.
    pending: Mutex<HashMap<String, Pending>>,
    /// Renders that failed, by key, and when.
    failed: Mutex<HashMap<String, Instant>>,
}

impl Renders {
    pub fn new(base: Option<String>, dir: PathBuf) -> Self {
        Self {
            inner: Arc::new(Inner {
                base: base.map(|base| base.trim_end_matches('/').to_owned()),
                dir,
                http: reqwest::Client::builder()
                    .timeout(RENDER_TIMEOUT)
                    .build()
                    .expect("a reqwest client builds"),
                pending: Mutex::default(),
                failed: Mutex::default(),
            }),
        }
    }

    fn path(&self, key: &str) -> PathBuf {
        self.inner.dir.join(format!("{key}.png"))
    }

    /// The render of `job` when it is on disk.
    pub async fn stored(&self, job: &Job) -> Option<Bytes> {
        tokio::fs::read(self.path(&job.key()))
            .await
            .ok()
            .map(Bytes::from)
    }

    /// The render of `job`: from disk, or drawn now if that takes at most
    /// `wait`. A render still being drawn then carries on in the background.
    pub async fn get(&self, job: &Job, wait: Duration) -> Option<Bytes> {
        if let Some(bytes) = self.stored(job).await {
            return Some(bytes);
        }
        let pending = self.start(job)?;
        tokio::time::timeout(wait, pending).await.ok().flatten()
    }

    /// The drawing of `job`, joined if one is under way; `None` when renders
    /// are off or this one failed lately.
    fn start(&self, job: &Job) -> Option<Pending> {
        let base = self.inner.base.clone()?;
        let key = job.key();
        let recently_failed = self
            .inner
            .failed
            .lock()
            .expect("lock poisoned")
            .get(&key)
            .is_some_and(|at| at.elapsed() < RETRY_AFTER);
        if recently_failed {
            return None;
        }
        let mut pending = self.inner.pending.lock().expect("lock poisoned");
        if let Some(running) = pending.get(&key) {
            return Some(running.clone());
        }
        let url = job.url(&base)?;
        let renders = self.clone();
        let task_key = key.clone();
        let handle = tokio::spawn(async move {
            let drawn = renders.draw(url, &task_key).await;
            if drawn.is_none() {
                renders
                    .inner
                    .failed
                    .lock()
                    .expect("lock poisoned")
                    .insert(task_key.clone(), Instant::now());
            }
            renders
                .inner
                .pending
                .lock()
                .expect("lock poisoned")
                .remove(&task_key);
            drawn
        });
        let shared = handle.map(Result::ok).map(Option::flatten).boxed().shared();
        pending.insert(key, shared.clone());
        Some(shared)
    }

    /// Asks the worker for a render and stores it.
    async fn draw(&self, url: reqwest::Url, key: &str) -> Option<Bytes> {
        let response = match self.inner.http.get(url).send().await {
            Ok(response) => response,
            Err(error) => {
                tracing::warn!(%error, "og: render worker unreachable");
                return None;
            }
        };
        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            tracing::warn!(%status, %message, "og: render failed");
            return None;
        }
        let bytes = response.bytes().await.ok()?;
        if !bytes.starts_with(PNG_SIGNATURE) {
            tracing::warn!("og: render worker sent something other than a PNG");
            return None;
        }
        super::store(&self.path(key), &bytes).await;
        Some(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_change_with_everything_a_render_depends_on() {
        let skinr = |look: &str| Job::Skinr {
            design_id: "abc".into(),
            ship_id: 587,
            look: look.into(),
        };
        assert_eq!(skinr("a").key(), skinr("a").key());
        assert_ne!(skinr("a").key(), skinr("b").key());
        assert_ne!(Job::Dna("a".into()).key(), Job::Dna("b".into()).key());
        assert_eq!(Job::Dna("a".into()).key().len(), 32);
    }

    #[test]
    fn worker_urls_carry_the_job() {
        let url = Job::Dna("mf4_t1:minmatarbase:minmatar:pattern?x;y".into())
            .url("http://renderer:3200")
            .unwrap();
        assert_eq!(url.path(), "/render");
        assert!(
            url.query()
                .unwrap()
                .contains("dna=mf4_t1%3Aminmatarbase%3Aminmatar%3Apattern%3Fx%3By")
        );
        let url = Job::Skinr {
            design_id: "abc".into(),
            ship_id: 587,
            look: String::new(),
        }
        .url("http://renderer:3200")
        .unwrap();
        assert_eq!(url.path(), "/render/skinr/abc");
        assert!(url.query().unwrap().starts_with("ship=587&size="));
    }
}
