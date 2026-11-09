//! Open Graph cards under `/og`, outside `/api` as in MutaMarket: each
//! renders its page's card as a 1200x630 PNG (see `crate::og`), and 404s
//! for an unknown ship, skin, design or designer.

use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

use super::AppState;
use super::api::{error, not_found};
use crate::og::{CONTENT_TYPE, Freshness, Rendered};

/// A card that shows everything its page has: unfurlers may keep it a
/// while, and its contents change rarely.
const FINAL_CACHE_CONTROL: &str = "public, max-age=3600, stale-while-revalidate=86400";
/// A stand-in card (its render is still being drawn): ask again soon.
const PROVISIONAL_CACHE_CONTROL: &str = "public, max-age=60";
/// A SKINR design id: 64 hex digits.
const DESIGN_ID_LENGTH: usize = 64;

/// The id in a `<id>.png` segment.
fn png_id<T: std::str::FromStr>(segment: &str) -> Option<T> {
    segment.strip_suffix(".png")?.parse().ok()
}

fn respond(card: sqlx::Result<Option<Rendered>>) -> Response {
    match card {
        Ok(Some(card)) => {
            let cache_control = match card.freshness {
                Freshness::Final => FINAL_CACHE_CONTROL,
                Freshness::Provisional => PROVISIONAL_CACHE_CONTROL,
            };
            (
                [
                    (header::CONTENT_TYPE, HeaderValue::from_static(CONTENT_TYPE)),
                    (
                        header::CACHE_CONTROL,
                        HeaderValue::from_static(cache_control),
                    ),
                ],
                card.png,
            )
                .into_response()
        }
        Ok(None) => error(StatusCode::NOT_FOUND, "Not found."),
        Err(cause) => super::api::database_error(cause),
    }
}

/// `/og/home.png`.
pub async fn home(State(state): State<AppState>) -> Response {
    respond(state.og.home(&state.pool).await.map(Some))
}

/// `/og/skinr.png`: the Paragon Hub.
pub async fn hub(State(state): State<AppState>) -> Response {
    respond(state.og.hub(&state.pool).await.map(Some))
}

/// `/og/ships/{type_id}.png`.
pub async fn ship(State(state): State<AppState>, Path(file): Path<String>) -> Response {
    let Some(ship_id) = png_id::<i32>(&file) else {
        return not_found().await;
    };
    respond(state.og.ship(&state.pool, ship_id).await)
}

/// `/og/ships/{type_id}/skins/{skin_id}.png`.
pub async fn skin(
    State(state): State<AppState>,
    Path((ship_id, file)): Path<(i32, String)>,
) -> Response {
    let Some(skin_id) = png_id::<i32>(&file) else {
        return not_found().await;
    };
    respond(state.og.skin(&state.pool, ship_id, skin_id).await)
}

/// `/og/skinr/{design_id}.png`, the full 64-hex id.
pub async fn design(State(state): State<AppState>, Path(file): Path<String>) -> Response {
    let Some(id) = file
        .strip_suffix(".png")
        .map(str::to_ascii_lowercase)
        .filter(|id| id.len() == DESIGN_ID_LENGTH && id.bytes().all(|b| b.is_ascii_hexdigit()))
    else {
        return not_found().await;
    };
    respond(state.og.design(&state.pool, &id).await)
}

/// `/og/designers/{character_id}.png`.
pub async fn designer(State(state): State<AppState>, Path(file): Path<String>) -> Response {
    let Some(character_id) = png_id::<i64>(&file) else {
        return not_found().await;
    };
    respond(state.og.designer(&state.pool, character_id).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_png_ids_are_taken() {
        assert_eq!(png_id::<i32>("587.png"), Some(587));
        assert_eq!(png_id::<i32>("587"), None);
        assert_eq!(png_id::<i32>("abc.png"), None);
        assert_eq!(png_id::<i32>("../587.png"), None);
    }
}
