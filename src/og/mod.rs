//! Open Graph cards, as in MutaMarket: each card is an SVG (`cards`) set in
//! the embedded Inter faces and rasterized with resvg, then kept on disk.
//!
//! A stored card's file name carries a hash of everything it shows (names,
//! prices, which render or thumbnail), so it is drawn again when any of that
//! changes. Cards that had to make do without a render (the worker is down
//! or slow, see `renders`) are served but not stored, and say so through
//! [`Freshness`] so unfurlers keep them only briefly.

mod cards;
mod fonts;
mod hexagon;
mod pages;
mod renders;
mod svg;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use resvg::{tiny_skia, usvg};
use sha2::{Digest, Sha256};

pub use cards::{HEIGHT, WIDTH};
pub use renders::RENDER_SIZE;
pub use svg::escape;

use renders::Renders;

/// The content type of every card.
pub const CONTENT_TYPE: &str = "image/png";

/// Where cards and renders are kept when `OG_CACHE_DIR` is unset.
const DEFAULT_CACHE_DIR: &str = "storage/og";
/// CCP's image server, for designer portraits.
const DEFAULT_IMAGE_SERVER: &str = "https://images.evetech.net";
/// How long a card waits for a render before it settles for a fallback:
/// link unfurlers give up after a few seconds.
const DEFAULT_RENDER_WAIT: Duration = Duration::from_secs(5);
/// A portrait fetch gets a short budget too.
const PORTRAIT_TIMEOUT: Duration = Duration::from_secs(5);
/// The portrait size fetched for a 128 px circle.
const PORTRAIT_SIZE: u32 = 256;
/// Bump to draw every card again after a change to the layout.
const CARD_VERSION: u32 = 2;

/// Whether a card is the one its page should have, or a stand-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// Everything it shows was there; stored, and fine to cache for long.
    Final,
    /// Drawn without a render or a portrait that is still coming.
    Provisional,
}

/// A card as served.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub png: Bytes,
    pub freshness: Freshness,
}

/// Where cards come from.
#[derive(Debug, Clone)]
pub struct OgConfig {
    pub cache_dir: PathBuf,
    /// Ship thumbnails (`<id>-dark.png`), the art of ship cards and the fallback.
    pub thumbnails: PathBuf,
    /// The render worker; `None` turns renders off.
    pub renderer_url: Option<String>,
    pub image_server: String,
    pub render_wait: Duration,
}

impl OgConfig {
    /// From `OG_CACHE_DIR`, `RENDERER_URL` and `IMAGE_SERVER_URL`, with the local defaults.
    pub fn from_env(thumbnails: PathBuf) -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        Self {
            cache_dir: var("OG_CACHE_DIR")
                .unwrap_or_else(|| DEFAULT_CACHE_DIR.to_owned())
                .into(),
            thumbnails,
            renderer_url: var("RENDERER_URL"),
            image_server: var("IMAGE_SERVER_URL")
                .unwrap_or_else(|| DEFAULT_IMAGE_SERVER.to_owned()),
            render_wait: DEFAULT_RENDER_WAIT,
        }
    }
}

/// The card maker: config, the render worker, and an HTTP client.
#[derive(Clone)]
pub struct Og {
    inner: Arc<Inner>,
}

struct Inner {
    config: OgConfig,
    renders: Renders,
    http: reqwest::Client,
}

impl Og {
    pub fn new(config: OgConfig) -> Self {
        let renders = Renders::new(
            config.renderer_url.clone(),
            config.cache_dir.join("renders"),
        );
        Self {
            inner: Arc::new(Inner {
                renders,
                http: reqwest::Client::builder()
                    .timeout(PORTRAIT_TIMEOUT)
                    .build()
                    .expect("a reqwest client builds"),
                config,
            }),
        }
    }

    fn config(&self) -> &OgConfig {
        &self.inner.config
    }

    fn renders(&self) -> &Renders {
        &self.inner.renders
    }

    /// A ship's dark thumbnail, with what identifies this rendering of it.
    async fn thumbnail(&self, ship_id: i32) -> Option<(Vec<u8>, String)> {
        let path = self.config().thumbnails.join(format!("{ship_id}-dark.png"));
        let meta = tokio::fs::metadata(&path).await.ok()?;
        let stamp = meta
            .modified()
            .ok()
            .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |since| since.as_secs());
        let bytes = tokio::fs::read(&path).await.ok()?;
        Some((bytes, format!("{ship_id}:{stamp}:{}", meta.len())))
    }

    /// A character's portrait, or `None` when the image server does not answer.
    async fn portrait(&self, character_id: i64) -> Option<Vec<u8>> {
        let base = self.config().image_server.trim_end_matches('/');
        let url = format!("{base}/characters/{character_id}/portrait?size={PORTRAIT_SIZE}");
        let response = self.inner.http.get(url).send().await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        let bytes = response.bytes().await.ok()?;
        (!bytes.is_empty()).then(|| bytes.to_vec())
    }

    /// Where a card with these contents is kept: `cards/<kind>/<id>-<hash>.png`.
    fn card_path(&self, kind: &str, id: &str, depends_on: &serde_json::Value) -> PathBuf {
        let mut hash = Sha256::new();
        hash.update(format!("v{CARD_VERSION}:{depends_on}"));
        let digest = renders::hex(&hash.finalize()[..8]);
        self.config()
            .cache_dir
            .join("cards")
            .join(kind)
            .join(format!("{id}-{digest}.png"))
    }

    /// The stored card at `path`, if there is one.
    async fn stored_card(path: &Path) -> Option<Rendered> {
        let png = tokio::fs::read(path).await.ok()?;
        Some(Rendered {
            png: png.into(),
            freshness: Freshness::Final,
        })
    }

    /// Rasterizes a card; stores it at `path` (and drops the same card's
    /// older files) when it is final.
    async fn finish(&self, card: cards::Card, freshness: Freshness, path: &Path) -> Rendered {
        let png: Bytes = tokio::task::spawn_blocking(move || rasterize(&cards::draw(&card)))
            .await
            .expect("rasterizing a card does not panic")
            .into();
        if freshness == Freshness::Final {
            forget_older(path).await;
            store(path, &png).await;
        }
        Rendered { png, freshness }
    }
}

/// Rasterizes an SVG card to PNG.
///
/// # Panics
///
/// When the SVG the card builder wrote does not parse or the PNG does not
/// encode: both are bugs in this module, not bad input.
fn rasterize(document: &str) -> Vec<u8> {
    let options = usvg::Options {
        fontdb: fonts::fonts().database(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(document, &options).expect("a card is valid SVG");
    let mut pixmap = tiny_skia::Pixmap::new(WIDTH, HEIGHT).expect("the card size is not zero");
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    pixmap.encode_png().expect("a card encodes as PNG")
}

/// Writes a file aside and moves it into place; a failure only costs the cache.
async fn store(path: &Path, bytes: &[u8]) {
    let Some(dir) = path.parent() else { return };
    let part = path.with_extension(format!("{}.part", std::process::id()));
    let written = async {
        tokio::fs::create_dir_all(dir).await?;
        tokio::fs::write(&part, bytes).await?;
        tokio::fs::rename(&part, path).await
    }
    .await;
    if let Err(error) = written {
        tracing::warn!(%error, path = %path.display(), "og: could not store");
        tokio::fs::remove_file(&part).await.ok();
    }
}

/// Removes the other files of the card at `path` (`<id>-<other hash>.png`).
async fn forget_older(path: &Path) {
    let (Some(dir), Some(name)) = (
        path.parent(),
        path.file_name().and_then(|name| name.to_str()),
    ) else {
        return;
    };
    let Some((id, _)) = name.rsplit_once('-') else {
        return;
    };
    let prefix = format!("{id}-");
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let other = entry.file_name();
        let Some(other) = other.to_str() else {
            continue;
        };
        if other != name && other.starts_with(&prefix) && other.ends_with(".png") {
            tokio::fs::remove_file(entry.path()).await.ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_rasterizes_at_open_graph_size() {
        let png = rasterize(&cards::draw(&cards::Card {
            eyebrow: "Frigate".into(),
            title: "Rifter".into(),
            subtitle: None,
            tint: svg::Color::constant("#e0875a"),
            art: cards::Art::None,
            portrait: None,
            badge: None,
            figure: None,
        }));
        let image = image::load_from_memory(&png).unwrap();
        assert_eq!((image.width(), image.height()), (WIDTH, HEIGHT));
    }

    #[tokio::test]
    async fn storing_a_card_drops_its_older_files_only() {
        let dir = std::env::temp_dir().join(format!("skinr-og-forget-{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        for name in ["587-old.png", "5870-other.png", "587-new.png"] {
            tokio::fs::write(dir.join(name), b"png").await.unwrap();
        }
        forget_older(&dir.join("587-new.png")).await;
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left, vec!["587-new.png", "5870-other.png"]);
        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }
}
