//! The axum router: the JSON API under `/api` and the CDN passthrough under
//! `/cdn`. The SvelteKit dev proxy and Caddy route these paths here and
//! everything else to the frontend, so the browser only ever sees one origin.

pub mod api;

use axum::Router;
use axum::extract::FromRef;
use axum::routing::get;
use sqlx::PgPool;

use crate::cdn::{self, CdnClient};

/// The default bind address of the API; the dev proxy and Caddy point at it.
const DEFAULT_BIND_ADDR: &str = "127.0.0.1:3100";

/// The listen address from `BIND_ADDR`, with the local default.
pub fn bind_addr() -> String {
    std::env::var("BIND_ADDR").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_owned())
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub cdn: CdnClient,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl FromRef<AppState> for CdnClient {
    fn from_ref(state: &AppState) -> Self {
        state.cdn.clone()
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .nest("/api", api::router())
        .route("/cdn/binaries/{*path}", get(cdn::binaries))
        .route("/cdn/resources/{*path}", get(cdn::resources))
        .fallback(api::not_found)
        .with_state(state)
}
