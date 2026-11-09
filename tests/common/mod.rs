//! Helpers for the suites that need Postgres and local mocks.
//!
//! Every test gets a database of its own on the server behind
//! `TEST_DATABASE_URL` (the docker compose Postgres by default), migrated
//! from scratch and dropped when the test passes.

// Not every suite uses every helper.
#![allow(dead_code)]

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use axum::response::Response;
use http_body_util::BodyExt;
use skinr::auth::sso::SsoClient;
use skinr::cdn::CdnClient;
use skinr::esi::client::EsiClient;
use skinr::game_files::GameFiles;
use skinr::server::{AppState, router};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{AssertSqlSafe, Connection, Executor, PgConnection, PgPool};
use tower::ServiceExt;

/// The docker compose Postgres (`docker compose up -d postgres`).
const DEFAULT_TEST_DATABASE_URL: &str = "postgres://skinr:skinr@127.0.0.1:5434/postgres";
/// Prefix of the databases tests create.
const PREFIX: &str = "skinr_test_";
/// Connections per test database.
const POOL_CONNECTIONS: u32 = 4;

static NEXT: AtomicU32 = AtomicU32::new(0);

fn server_options() -> PgConnectOptions {
    let url =
        std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| DEFAULT_TEST_DATABASE_URL.to_owned());
    PgConnectOptions::from_str(&url).expect("TEST_DATABASE_URL parses")
}

async fn server() -> PgConnection {
    PgConnection::connect_with(&server_options())
        .await
        .expect("Postgres not reachable: start it with `docker compose up -d postgres`")
}

/// Keeps a test's database while the test runs; drops it when the test
/// passes. A failed test's database stays to be looked at.
pub struct TestDb {
    name: String,
}

impl Drop for TestDb {
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }
        let name = std::mem::take(&mut self.name);
        // Drop runs inside the test's runtime, which cannot block on it.
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("a runtime")
                .block_on(async {
                    let mut conn = server().await;
                    conn.execute(AssertSqlSafe(format!(
                        "drop database if exists \"{name}\" with (force)"
                    )))
                    .await
                    .ok();
                });
        })
        .join()
        .ok();
    }
}

/// A migrated database of the test's own.
pub async fn database() -> (PgPool, TestDb) {
    let name = format!(
        "{PREFIX}{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let mut conn = server().await;
    conn.execute(AssertSqlSafe(format!(
        "drop database if exists \"{name}\" with (force)"
    )))
    .await
    .expect("drop a leftover test database");
    conn.execute(AssertSqlSafe(format!("create database \"{name}\"")))
        .await
        .expect("create a test database");
    conn.close().await.ok();

    let pool = PgPoolOptions::new()
        .max_connections(POOL_CONNECTIONS)
        .connect_with(server_options().database(&name))
        .await
        .expect("connect to the test database");
    skinr::db::migrate(&pool).await.expect("migrations");
    (pool, TestDb { name })
}

/// Serves `app` on an ephemeral port; returns its base URL.
pub async fn serve(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

/// The real router over `pool`, with ESI (and the SSO, if given) at `mock`.
pub fn app(pool: PgPool, mock: &str, sso: Option<SsoClient>) -> Router {
    router(AppState {
        pool,
        cdn: CdnClient::new("http://127.0.0.1:1", "http://127.0.0.1:1"),
        game_files: GameFiles::new(
            CdnClient::new("http://127.0.0.1:1", "http://127.0.0.1:1"),
            std::env::temp_dir(),
        ),
        thumbnails: std::env::temp_dir(),
        esi: EsiClient::new(mock, "skinr.space-test/1.0"),
        sso,
        og: skinr::og::Og::new(skinr::og::OgConfig::from_env(std::env::temp_dir())),
        public_origin: "https://skinr.space".to_owned(),
        drawing: None,
        debug: false,
    })
}

pub async fn send(app: &Router, request: Request<Body>) -> Response {
    app.clone().oneshot(request).await.unwrap()
}

pub async fn get(app: &Router, path: &str, cookie: Option<&str>) -> Response {
    let mut request = Request::get(path);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    send(app, request.body(Body::empty()).unwrap()).await
}

pub async fn json(response: Response) -> serde_json::Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}
