//! The debug tools (`/api/debug`, `DEBUG_TOOLS=1`): a livery's thumbnails,
//! drawn on request through a mock render worker on an ephemeral port, then
//! served under /api/thumbnails. Needs Postgres (`docker compose up -d postgres`).

mod common;

use std::path::PathBuf;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Value, json};
use skinr::cdn::CdnClient;
use skinr::game_files::GameFiles;
use skinr::server::{AppState, router};
use skinr::thumbnails::Thumbnails;
use sqlx::PgPool;

const RIFTER: i32 = 587;
const SLASHER: i32 = 585;
/// A Rifter skin, and a Slasher one.
const RIFTER_SKIN: i32 = 1001;
const SLASHER_SKIN: i32 = 2002;

async fn seed(pool: &PgPool) {
    for statement in [
        "insert into ship_groups (id, name) values (25, 'Frigate')",
        "insert into ships (id, name, group_id, sof_hull, sof_faction, sof_race)
         values (587, 'Rifter', 25, 'mf4_t1', 'minmatarbase', 'minmatar'),
                (585, 'Slasher', 25, 'mf2_t1', 'minmatarbase', 'minmatar')",
        "insert into ship_skins (ship_id, skin_id, name, sof_faction, sof_commands)
         values (587, 1001, 'Rifter Headhunter', 'bloodraider', 'pattern?headhunter;a;b'),
                (585, 2002, 'Slasher Glory', 'minmatarbase', '')",
    ] {
        sqlx::query(statement).execute(pool).await.unwrap();
    }
}

fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::RgbaImage::from_pixel(4, 4, image::Rgba([10, 20, 30, 255]))
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

/// A mock worker that draws anything.
async fn worker() -> String {
    let app = Router::new().route(
        "/thumbnail",
        post(|| async {
            let encoded = BASE64.encode(png());
            json!({"pngs": {"dark": encoded, "light": encoded}, "used": [], "missing": []})
                .to_string()
        }),
    );
    common::serve(app).await
}

fn directory(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("skinr-debug-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    dir
}

fn app(pool: PgPool, dir: PathBuf, renderer: Option<&str>, debug: bool) -> Router {
    router(AppState {
        drawing: renderer.map(|base| Thumbnails::new(pool.clone(), dir.clone(), base)),
        pool,
        cdn: CdnClient::new("http://127.0.0.1:1", "http://127.0.0.1:1"),
        game_files: GameFiles::new(
            CdnClient::new("http://127.0.0.1:1", "http://127.0.0.1:1"),
            std::env::temp_dir(),
        ),
        thumbnails: dir,
        esi: skinr::esi::client::EsiClient::new("http://127.0.0.1:1", "skinr.space-test/1.0"),
        sso: None,
        og: skinr::og::Og::new(skinr::og::OgConfig::from_env(std::env::temp_dir())),
        public_origin: "https://skinr.space".to_owned(),
        debug,
    })
}

async fn post_to(app: &Router, path: &str) -> axum::response::Response {
    common::send(app, Request::post(path).body(Body::empty()).unwrap()).await
}

#[tokio::test]
async fn a_skin_thumbnail_is_drawn_on_request_and_served() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let dir = directory("skin");
    let base = worker().await;
    let app = app(pool, dir.clone(), Some(&base), true);
    let path = format!("/api/debug/thumbnails/{RIFTER}/{RIFTER_SKIN}");

    let before = common::json(common::get(&app, &path, None).await).await;
    assert_eq!(
        before["dna"],
        "mf4_t1:bloodraider:minmatar:pattern?headhunter;a;b"
    );
    assert_eq!(before["current"], false);
    assert_eq!(before["dark"], Value::Null);
    assert_eq!(before["can_draw"], true);

    let response = post_to(&app, &path).await;
    assert_eq!(response.status(), StatusCode::OK);
    let after = common::json(response).await;
    assert_eq!(after["current"], true);
    assert_eq!(after["drawn_from"], after["dna"]);
    let dark = after["dark"].as_str().unwrap();
    assert!(
        dark.starts_with(&format!(
            "/api/thumbnails/skins/{RIFTER}_{RIFTER_SKIN}-dark.png?v="
        )),
        "{dark}"
    );
    assert!(
        dir.join(format!("skins/{RIFTER}_{RIFTER_SKIN}.dna"))
            .exists()
    );

    // The PNG it points at is served, and as a WebP at card size.
    let served = common::get(
        &app,
        &format!("/api/thumbnails/skins/{RIFTER}_{RIFTER_SKIN}-light.png"),
        None,
    )
    .await;
    assert_eq!(served.status(), StatusCode::OK);
    let webp = common::get(
        &app,
        &format!("/api/thumbnails/skins/{RIFTER}_{RIFTER_SKIN}-dark-256.webp"),
        None,
    )
    .await;
    assert_eq!(webp.status(), StatusCode::OK);
    // A skin is not mistaken for a ship.
    let ship = common::get(
        &app,
        &format!("/api/thumbnails/{RIFTER_SKIN}-dark.png"),
        None,
    )
    .await;
    assert_eq!(ship.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_default_livery_uses_the_ship_thumbnails() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let dir = directory("default");
    let base = worker().await;
    let app = app(pool, dir.clone(), Some(&base), true);

    let drawn =
        common::json(post_to(&app, &format!("/api/debug/thumbnails/{SLASHER}/default")).await)
            .await;
    assert_eq!(drawn["dna"], "mf2_t1:minmatarbase:minmatar");
    assert!(
        drawn["light"]
            .as_str()
            .unwrap()
            .starts_with(&format!("/api/thumbnails/{SLASHER}-light.png?v="))
    );
    assert!(dir.join(format!("{SLASHER}.files")).exists());
}

#[tokio::test]
async fn only_a_ships_own_skins_are_found() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let base = worker().await;
    let app = app(pool, directory("other"), Some(&base), true);
    for path in [
        format!("/api/debug/thumbnails/{RIFTER}/{SLASHER_SKIN}"),
        format!("/api/debug/thumbnails/{RIFTER}/glory"),
        "/api/debug/thumbnails/1/default".to_owned(),
    ] {
        assert_eq!(
            post_to(&app, &path).await.status(),
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
}

#[tokio::test]
async fn without_a_worker_nothing_is_drawn() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let app = app(pool, directory("no-worker"), None, true);
    let path = format!("/api/debug/thumbnails/{RIFTER}/{RIFTER_SKIN}");
    let info = common::json(common::get(&app, &path, None).await).await;
    assert_eq!(info["can_draw"], false);
    assert_eq!(
        post_to(&app, &path).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn the_tools_are_off_unless_asked_for() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let base = worker().await;
    let dir = directory("off");
    let app = app(pool, dir.clone(), Some(&base), false);
    let path = format!("/api/debug/thumbnails/{RIFTER}/{RIFTER_SKIN}");
    assert_eq!(post_to(&app, &path).await.status(), StatusCode::NOT_FOUND);
    assert!(!dir.join("skins").exists());
}

#[tokio::test]
async fn a_design_thumbnail_is_drawn_on_request() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let design = "cd".repeat(32);
    sqlx::query("insert into esi_characters (id, name) values (2112000010, 'Alice')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "insert into skinr_designs (id, name, creator_id, ship_type_id, layout)
         values ($1, 'Rust Bucket', 2112000010, 587, '{}')",
    )
    .bind(&design)
    .execute(&pool)
    .await
    .unwrap();
    let dir = directory("design");
    let base = worker().await;
    let app = app(pool, dir.clone(), Some(&base), true);

    let drawn = common::json(
        post_to(
            &app,
            &format!("/api/debug/thumbnails/{RIFTER}/skinr/{design}"),
        )
        .await,
    )
    .await;
    assert_eq!(drawn["current"], true);
    assert_eq!(drawn["dna"], "mf4_t1:minmatarbase:minmatar|{}");
    let dark = drawn["dark"].as_str().unwrap();
    assert!(dark.starts_with(&format!("/api/thumbnails/designs/{design}-dark.png?v=")));
    let served = common::get(
        &app,
        &format!("/api/thumbnails/designs/{design}-dark.png"),
        None,
    )
    .await;
    assert_eq!(served.status(), StatusCode::OK);
    // Not on another hull.
    let other = post_to(
        &app,
        &format!("/api/debug/thumbnails/{SLASHER}/skinr/{design}"),
    )
    .await;
    assert_eq!(other.status(), StatusCode::NOT_FOUND);
}
