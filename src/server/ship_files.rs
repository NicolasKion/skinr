//! `/api/ships/{id}/files`: every `res:/` path a ship's render used.
//!
//! [`crate::thumbnails`] records them beside each ship's thumbnails
//! (`<id>.files`, a JSON array). The page fetches them all as soon
//! as a ship is picked, instead of finding them one after another as the
//! runtime walks from the hull to its textures and shaders.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};

use super::AppState;

/// The lists change when the thumbnails are re-rendered, so they are cached briefly.
const CACHE_CONTROL: &str = "public, max-age=300";
/// The scheme every listed path carries.
const RES_ROOT: &str = "res:/";

/// A ship's recorded paths; empty for a ship not rendered yet or an
/// unreadable list, since preloading is only a head start.
pub async fn serve(State(state): State<AppState>, Path(id): Path<u32>) -> Response {
    let paths: Vec<String> = tokio::fs::read(state.thumbnails.join(format!("{id}.files")))
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|path| path.starts_with(RES_ROOT) && !path.contains(".."))
        .collect();
    (
        [(
            header::CACHE_CONTROL,
            HeaderValue::from_static(CACHE_CONTROL),
        )],
        Json(paths),
    )
        .into_response()
}
