//! Tools for working on the site, under `/api/debug` and only when
//! `DEBUG_TOOLS=1`: they start renders on request.
//!
//! A livery's thumbnails, where the livery is `default`, a skin id, or
//! `skinr/<design id>`:
//!
//! - `GET /api/debug/thumbnails/{ship}/{livery}` says what there is for it
//! - `POST` to the same draws them now (waiting for the render worker) and
//!   says the same
//!
//! A skin's or design's drawing is also what its Open Graph card shows until
//! the card's own render is there (`og::pages`).

use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::Serialize;

use super::AppState;
use super::api::{database_error, error};
use crate::dna;
use crate::thumbnails::{Subject, THEMES};

/// The livery segment that names a ship's default livery.
const DEFAULT_LIVERY: &str = "default";

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/thumbnails/{ship}/{livery}",
            get(livery_status).post(draw_livery),
        )
        .route(
            "/thumbnails/{ship}/skinr/{design}",
            get(design_status).post(draw_design),
        )
}

/// A livery's thumbnails.
#[derive(Serialize)]
struct ThumbnailInfo {
    /// What the livery is now: its DNA, or a design's look.
    dna: String,
    /// What the thumbnails on disk were drawn from.
    drawn_from: Option<String>,
    /// Whether both themes are on disk and drawn from `dna`.
    current: bool,
    /// Each theme's PNG, with its write time in the query so a redraw is
    /// fetched again.
    dark: Option<String>,
    light: Option<String>,
    /// Whether this API can draw them (it has a render worker).
    can_draw: bool,
}

#[derive(sqlx::FromRow)]
struct LiveryRow {
    sof_hull: String,
    sof_faction: String,
    sof_race: String,
    /// A skin's or the ship's material set; a design's layout.
    extra: String,
}

const NO_SUCH_LIVERY: &str = "No such livery on that ship.";

/// The subject and DNA of `livery` (`default` or a skin id) on `ship`.
async fn livery(state: &AppState, ship: i32, livery: &str) -> Result<(Subject, String), Response> {
    let (subject, row) = if livery == DEFAULT_LIVERY {
        let row = sqlx::query_as::<_, LiveryRow>(
            "select sof_hull, sof_faction, sof_race, sof_commands as extra from ships where id = $1",
        )
        .bind(ship)
        .fetch_optional(&state.pool)
        .await
        .map_err(database_error)?;
        (Subject::Ship(ship), row)
    } else {
        let skin: i32 = livery
            .parse()
            .map_err(|_| error(StatusCode::NOT_FOUND, "Not a livery."))?;
        let row = sqlx::query_as::<_, LiveryRow>(
            "select s.sof_hull, k.sof_faction, s.sof_race, k.sof_commands as extra
             from ship_skins k join ships s on s.id = k.ship_id
             where k.ship_id = $1 and k.skin_id = $2",
        )
        .bind(ship)
        .bind(skin)
        .fetch_optional(&state.pool)
        .await
        .map_err(database_error)?;
        (Subject::Skin { ship, skin }, row)
    };
    let row = row.ok_or_else(|| error(StatusCode::NOT_FOUND, NO_SUCH_LIVERY))?;
    let dna = dna::livery(&row.sof_hull, &row.sof_faction, &row.sof_race, &row.extra);
    Ok((subject, dna))
}

/// The subject and look of SKINR design `design` on `ship`.
async fn design(state: &AppState, ship: i32, design: &str) -> Result<(Subject, String), Response> {
    let row = sqlx::query_as::<_, LiveryRow>(
        "select s.sof_hull, s.sof_faction, s.sof_race, d.layout as extra
         from skinr_designs d join ships s on s.id = d.ship_type_id
         where d.id = $1 and d.ship_type_id = $2",
    )
    .bind(design)
    .bind(ship)
    .fetch_optional(&state.pool)
    .await
    .map_err(database_error)?
    .ok_or_else(|| error(StatusCode::NOT_FOUND, NO_SUCH_LIVERY))?;
    let look = dna::design_look(&row.sof_hull, &row.sof_faction, &row.sof_race, &row.extra);
    let subject = Subject::Design {
        id: design.to_owned(),
        ship,
    };
    Ok((subject, look))
}

async fn info(state: &AppState, subject: &Subject, dna: String) -> ThumbnailInfo {
    let Some(drawing) = &state.drawing else {
        return ThumbnailInfo {
            dna,
            drawn_from: None,
            current: false,
            dark: None,
            light: None,
            can_draw: false,
        };
    };
    let status = drawing.status(subject).await;
    let [dark, light] = [0, 1].map(|index| {
        status.written[index].map(|at| {
            let version = at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            format!(
                "/api/thumbnails/{}-{}.png?v={version}",
                subject.stem(),
                THEMES[index]
            )
        })
    });
    let current =
        status.drawn_from.as_deref() == Some(dna.as_str()) && dark.is_some() && light.is_some();
    ThumbnailInfo {
        dna,
        drawn_from: status.drawn_from,
        current,
        dark,
        light,
        can_draw: true,
    }
}

/// Says what there is for a livery that was found.
async fn status(state: &AppState, found: Result<(Subject, String), Response>) -> Response {
    match found {
        Ok((subject, key)) => axum::Json(info(state, &subject, key).await).into_response(),
        Err(response) => response,
    }
}

/// Draws a livery that was found, then says what there is.
async fn draw(state: &AppState, found: Result<(Subject, String), Response>) -> Response {
    let (subject, key) = match found {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Some(drawing) = &state.drawing else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "RENDERER_URL is not set.");
    };
    let started = SystemTime::now();
    if let Err(drawn) = drawing.draw(&subject, &key).await {
        tracing::warn!(?subject, %key, error = %drawn, "debug: thumbnail failed");
        return error(StatusCode::BAD_GATEWAY, &drawn.to_string());
    }
    tracing::info!(
        ?subject,
        ms = started.elapsed().unwrap_or_default().as_millis(),
        "debug: thumbnail drawn"
    );
    axum::Json(info(state, &subject, key).await).into_response()
}

async fn livery_status(
    State(state): State<AppState>,
    Path((ship, segment)): Path<(i32, String)>,
) -> Response {
    status(&state, livery(&state, ship, &segment).await).await
}

async fn draw_livery(
    State(state): State<AppState>,
    Path((ship, segment)): Path<(i32, String)>,
) -> Response {
    draw(&state, livery(&state, ship, &segment).await).await
}

async fn design_status(
    State(state): State<AppState>,
    Path((ship, id)): Path<(i32, String)>,
) -> Response {
    status(&state, design(&state, ship, &id).await).await
}

async fn draw_design(
    State(state): State<AppState>,
    Path((ship, id)): Path<(i32, String)>,
) -> Response {
    draw(&state, design(&state, ship, &id).await).await
}
