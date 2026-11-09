//! The axum router: the JSON API under `/api`, and under `/cdn` the CDN
//! passthrough and game files by path. The SvelteKit dev proxy and Caddy
//! route these paths here and everything else to the frontend, so the
//! browser only ever sees one origin.

pub mod api;
pub mod auth;
pub mod debug;
pub mod me;
pub mod og;
pub mod ship_files;
pub mod sitemap;
pub mod skinr;
pub mod thumbnails;

use std::path::PathBuf;

use axum::Router;
use axum::extract::FromRef;
use axum::routing::{get, post};
use sqlx::PgPool;

use crate::auth::sso::SsoClient;
use crate::cdn::{self, CdnClient};
use crate::esi::client::EsiClient;
use crate::game_files::{self, GameFiles};

/// The default bind address of the API; the dev proxy and Caddy point at it.
const DEFAULT_BIND_ADDR: &str = "127.0.0.1:3100";

/// Where [`crate::thumbnails`] keeps ship thumbnails.
const DEFAULT_THUMBNAILS_DIR: &str = "storage/thumbnails";

/// The thumbnail directory from `THUMBNAILS_DIR`, with the local default.
pub fn thumbnails_dir() -> PathBuf {
    std::env::var("THUMBNAILS_DIR")
        .unwrap_or_else(|_| DEFAULT_THUMBNAILS_DIR.to_owned())
        .into()
}

/// The listen address from `BIND_ADDR`, with the local default.
pub fn bind_addr() -> String {
    std::env::var("BIND_ADDR").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_owned())
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub cdn: CdnClient,
    /// Game files by `res:/` path, under /cdn/build, /cdn/res and /cdn/list.
    pub game_files: GameFiles,
    /// Rendered ship thumbnails (`<id>-<theme>.png`), served under /api/thumbnails.
    pub thumbnails: PathBuf,
    /// ESI, for a logged-in character's SKINR data (`me`).
    pub esi: EsiClient,
    /// EVE SSO; `None` while the app has no registration, which turns login off.
    pub sso: Option<SsoClient>,
    /// Open Graph cards, under /og.
    pub og: crate::og::Og,
    /// The site's origin (`PUBLIC_ORIGIN`), for the sitemap's absolute URLs.
    pub public_origin: String,
    /// Draws thumbnails through the render worker; `None` without `RENDERER_URL`.
    pub drawing: Option<crate::thumbnails::Thumbnails>,
    /// Mounts the debug tools under /api/debug (`DEBUG_TOOLS=1`). They start
    /// renders on request, so production leaves them off.
    pub debug: bool,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl FromRef<AppState> for GameFiles {
    fn from_ref(state: &AppState) -> Self {
        state.game_files.clone()
    }
}

impl FromRef<AppState> for CdnClient {
    fn from_ref(state: &AppState) -> Self {
        state.cdn.clone()
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .nest(
            "/api",
            if state.debug {
                api::router().nest("/debug", debug::router())
            } else {
                api::router()
            },
        )
        // Log in with EVE, under /eve as in MutaMarket (the SSO app's callback).
        .route("/eve/login", get(auth::login))
        .route("/eve/callback", get(auth::callback))
        .route("/eve/logout", post(auth::logout))
        // Open Graph cards and the sitemap, outside /api as in MutaMarket.
        .route("/og/home.png", get(og::home))
        .route("/og/skinr.png", get(og::hub))
        .route("/og/ships/{file}", get(og::ship))
        .route("/og/ships/{ship}/skins/{file}", get(og::skin))
        .route("/og/skinr/{file}", get(og::design))
        .route("/og/designers/{file}", get(og::designer))
        .route("/sitemap.xml", get(sitemap::sitemap))
        .route("/cdn/binaries/{*path}", get(cdn::binaries))
        .route("/cdn/resources/{*path}", get(cdn::resources))
        .route("/cdn/build", get(game_files::build))
        .route("/cdn/res/{build}/{*path}", get(game_files::file))
        .route("/cdn/list/{build}/{*folder}", get(game_files::list))
        .fallback(api::not_found)
        .with_state(state)
}
