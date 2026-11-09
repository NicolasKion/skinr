//! Ship thumbnails under `/api/thumbnails`: the renders [`crate::thumbnails`]
//! keeps (`<id>-<theme>.png`, 512 px, transparent), and WebP copies at the sizes cards draw them
//! (`<id>-<theme>-<size>.webp`), a tenth of the bytes. A copy is made from
//! its PNG on first request and kept in memory until the PNG changes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use axum::body::Bytes;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use image::imageops::{self, FilterType};
use image::{ImageFormat, RgbaImage};

use super::AppState;
use super::api::{error, not_found};
use crate::thumbnails::{DESIGN_FOLDER, SKIN_FOLDER, THEMES};

/// Thumbnails are re-rendered when the look changes, so they are cached briefly.
const CACHE_CONTROL: &str = "public, max-age=300";
/// The WebP widths, in px: a card (13rem) on a 1x and a 2x screen.
const WEBP_SIZES: [u32; 2] = [256, 512];
/// Lossy WebP quality, 0 to 100: no visible loss on a ship at card size.
const WEBP_QUALITY: f32 = 82.0;

/// What a request names: a thumbnail's PNG, and the WebP width wanted.
struct Wanted {
    png: String,
    webp: Option<u32>,
}

/// What names a thumbnail in a folder: a type or skin id, or a design id.
#[derive(Clone, Copy)]
enum Ids {
    Numeric,
    /// A type id and a skin id, `<ship>_<skin>` (a skin can fit several hulls).
    ShipSkin,
    /// 64 hex digits, a SKINR design's id.
    Design,
}

impl Ids {
    fn allow(self, id: &str) -> bool {
        match self {
            Self::Numeric => id.parse::<u32>().is_ok(),
            Self::ShipSkin => id
                .split_once('_')
                .is_some_and(|(ship, skin)| Self::Numeric.allow(ship) && Self::Numeric.allow(skin)),
            Self::Design => {
                id.len() == DESIGN_ID_LENGTH && id.bytes().all(|byte| byte.is_ascii_hexdigit())
            }
        }
    }
}

/// The length of a SKINR design's id, in hex digits.
const DESIGN_ID_LENGTH: usize = 64;

/// `<id>-<theme>.png` or `<id>-<theme>-<size>.webp`; nothing else, so a
/// request can never reach outside the directory.
fn parse(file: &str, ids: Ids) -> Option<Wanted> {
    let thumbnail = |name: &str| {
        let (id, theme) = name.split_once('-')?;
        (ids.allow(id) && THEMES.contains(&theme)).then(|| format!("{name}.png"))
    };
    if let Some(name) = file.strip_suffix(".png") {
        return Some(Wanted {
            png: thumbnail(name)?,
            webp: None,
        });
    }
    let (name, size) = file.strip_suffix(".webp")?.rsplit_once('-')?;
    let size = size
        .parse::<u32>()
        .ok()
        .filter(|size| WEBP_SIZES.contains(size))?;
    Some(Wanted {
        png: thumbnail(name)?,
        webp: Some(size),
    })
}

/// WebP copies made so far, by PNG path and width, with the PNG's mtime.
type Copies = HashMap<(PathBuf, u32), (SystemTime, Bytes)>;
static COPIES: LazyLock<Mutex<Copies>> = LazyLock::new(Mutex::default);

/// A thumbnail's WebP copy at `size`, made now unless one of this PNG is kept.
async fn webp_copy(png: &Path, size: u32) -> Result<Bytes, Response> {
    let missing = || error(StatusCode::NOT_FOUND, "Thumbnail not found.");
    let modified = tokio::fs::metadata(png)
        .await
        .and_then(|meta| meta.modified())
        .map_err(|_| missing())?;
    let key = (png.to_owned(), size);
    let kept = COPIES
        .lock()
        .expect("lock poisoned")
        .get(&key)
        .filter(|(at, _)| *at == modified)
        .map(|(_, bytes)| bytes.clone());
    if let Some(bytes) = kept {
        return Ok(bytes);
    }
    let source = tokio::fs::read(png).await.map_err(|_| missing())?;
    let bytes: Bytes = tokio::task::spawn_blocking(move || encode(&source, size))
        .await
        .ok()
        .flatten()
        .ok_or_else(|| {
            tracing::warn!(png = %png.display(), "thumbnails: unreadable PNG");
            error(StatusCode::INTERNAL_SERVER_ERROR, "Thumbnail unreadable.")
        })?
        .into();
    COPIES
        .lock()
        .expect("lock poisoned")
        .insert(key, (modified, bytes.clone()));
    Ok(bytes)
}

/// A transparent PNG as a lossy WebP `size` px wide. Scaled with
/// premultiplied alpha, so the hull's edges do not pick up the black of the
/// transparent pixels around them.
fn encode(png: &[u8], size: u32) -> Option<Vec<u8>> {
    let mut image = image::load_from_memory_with_format(png, ImageFormat::Png)
        .ok()?
        .to_rgba8();
    for pixel in image.pixels_mut() {
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel.0[..3] {
            *channel = channel_byte((u32::from(*channel) * alpha + 127) / 255);
        }
    }
    let (width, height) = image.dimensions();
    let mut image: RgbaImage = if width == size {
        image
    } else {
        let scaled_height =
            u32::try_from(u64::from(height) * u64::from(size) / u64::from(width.max(1)))
                .unwrap_or(size);
        imageops::resize(&image, size, scaled_height.max(1), FilterType::Lanczos3)
    };
    for pixel in image.pixels_mut() {
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel.0[..3] {
            // A fully transparent pixel keeps its (black) colour.
            if let Some(straight) = (u32::from(*channel) * 255 + alpha / 2).checked_div(alpha) {
                *channel = channel_byte(straight);
            }
        }
    }
    let encoded = webp::Encoder::from_rgba(image.as_raw(), image.width(), image.height())
        .encode(WEBP_QUALITY);
    Some(encoded.to_vec())
}

/// A colour value as a byte, clamped to its range.
fn channel_byte(value: u32) -> u8 {
    u8::try_from(value).unwrap_or(u8::MAX)
}

/// `/api/thumbnails/{file}`: a ship in its default livery.
pub async fn serve(State(state): State<AppState>, UrlPath(file): UrlPath<String>) -> Response {
    serve_from(&state.thumbnails, &file, Ids::Numeric).await
}

/// `/api/thumbnails/skins/{file}`: a ship in one of its skins
/// (`<type id>_<skin id>-<theme>.png`).
pub async fn serve_skin(State(state): State<AppState>, UrlPath(file): UrlPath<String>) -> Response {
    serve_from(&state.thumbnails.join(SKIN_FOLDER), &file, Ids::ShipSkin).await
}

/// `/api/thumbnails/designs/{file}`: a ship wearing a SKINR design (`<design id>-<theme>.png`).
pub async fn serve_design(
    State(state): State<AppState>,
    UrlPath(file): UrlPath<String>,
) -> Response {
    serve_from(&state.thumbnails.join(DESIGN_FOLDER), &file, Ids::Design).await
}

async fn serve_from(dir: &Path, file: &str, ids: Ids) -> Response {
    let Some(wanted) = parse(file, ids) else {
        return not_found().await;
    };
    let png = dir.join(&wanted.png);
    let (content_type, body) = match wanted.webp {
        None => match tokio::fs::read(&png).await {
            Ok(bytes) => ("image/png", Bytes::from(bytes)),
            Err(_) => return error(StatusCode::NOT_FOUND, "Thumbnail not found."),
        },
        Some(size) => match webp_copy(&png, size).await {
            Ok(bytes) => ("image/webp", bytes),
            Err(response) => return response,
        },
    };
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(content_type)),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static(CACHE_CONTROL),
            ),
        ],
        body,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_only_thumbnails() {
        let png = |file| parse(file, Ids::Numeric).map(|wanted| (wanted.png, wanted.webp));
        assert_eq!(png("17715-dark.png"), Some(("17715-dark.png".into(), None)));
        assert_eq!(
            png("17715-light-256.webp"),
            Some(("17715-light.png".into(), Some(256)))
        );
        assert_eq!(png("17715-light-300.webp"), None);
        assert_eq!(png("17715-sepia.png"), None);
        assert_eq!(png("../secret-dark.png"), None);
        assert_eq!(png("17715-dark.webp"), None);

        let design = "ab".repeat(32);
        let named = |file: &str| parse(file, Ids::Design).map(|wanted| wanted.png);
        assert_eq!(
            named(&format!("{design}-dark.png")),
            Some(format!("{design}-dark.png"))
        );
        assert_eq!(named("17715-dark.png"), None);
        let skin = |file: &str| parse(file, Ids::ShipSkin).map(|wanted| wanted.png);
        assert_eq!(skin("587_1001-dark.png"), Some("587_1001-dark.png".into()));
        assert_eq!(skin("587-dark.png"), None);
        assert_eq!(skin("587_x-dark.png"), None);
        assert_eq!(named(&format!("{}-dark.png", "zz".repeat(32))), None);
    }

    #[test]
    fn keeps_a_transparent_edge_clean() {
        // One opaque white pixel beside transparent black ones.
        let mut source = RgbaImage::new(4, 4);
        source.put_pixel(1, 1, image::Rgba([255, 255, 255, 255]));
        let mut png = Vec::new();
        source
            .write_to(&mut std::io::Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();

        let webp = encode(&png, 2).unwrap();
        assert_eq!(&webp[..4], b"RIFF");
        assert_eq!(&webp[8..12], b"WEBP");

        // Scaled down, the white pixel blends into a partly transparent one.
        // Scaled straight, the transparent black would grey it out.
        let decoded = webp::Decoder::new(&webp).decode().unwrap();
        let rgba = decoded.to_vec();
        let (red, alpha) = (rgba[0], rgba[3]);
        assert!(alpha > 0 && alpha < 255, "alpha {alpha}");
        assert!(red > 200, "red {red}");
    }
}
