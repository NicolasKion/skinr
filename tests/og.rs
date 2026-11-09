//! Open Graph cards (`/og/...`) and the sitemap, through the real router over
//! a seeded database, with the render worker and the image server replaced
//! by local mocks. Needs Postgres (`docker compose up -d postgres`).

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::Router;
use axum::extract::{Path, RawQuery, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use http_body_util::BodyExt;
use image::{ImageFormat, Rgba, RgbaImage};
use skinr::cdn::CdnClient;
use skinr::esi::client::EsiClient;
use skinr::game_files::GameFiles;
use skinr::og::{Og, OgConfig};
use skinr::server::{AppState, router};
use sqlx::PgPool;

const RIFTER: i32 = 587;
const KRUSUAL: i32 = 34;
const DESIGN: &str = "573b75f1393b5fb651dedda179977ab630d472641c86a3a1ba974b5a56b9d42b";
const ALICE: i64 = 2_112_000_010;

/// What the mock worker draws, and what the thumbnail shows: told apart by
/// the pixel in the middle of a card's art.
const RENDER_COLOR: [u8; 4] = [255, 0, 255, 255];
const THUMBNAIL_COLOR: [u8; 4] = [0, 200, 0, 255];
/// The middle of a card's art (`src/og/cards.rs`, `ART_CENTER`).
const ART_PIXEL: (u32, u32) = (900, 330);

async fn seed(pool: &PgPool) {
    for statement in [
        "insert into races (id, name) values (2, 'Minmatar')",
        "insert into ship_groups (id, name) values (25, 'Frigate')",
        "insert into ships (id, name, group_id, race_id, sof_hull, sof_faction, sof_race)
         values (587, 'Rifter', 25, 2, 'mf4_t1', 'minmatarbase', 'minmatar')",
        "insert into ship_skins (ship_id, skin_id, name, sof_faction, sof_commands, color_primary)
         values (587, 34, 'Rifter Krusual SKIN', 'krusual', 'pattern?krusual;none;none', '#aa4422')",
        "insert into esi_characters (id, name) values (2112000010, 'Alice O''Neil')",
        "insert into skinr_designs (id, name, creator_id, ship_type_id, layout) values
            ('573b75f1393b5fb651dedda179977ab630d472641c86a3a1ba974b5a56b9d42b', 'Rust Bucket',
             2112000010, 587, '{\"slots\": [{\"id\": 1, \"configuration\": {\"nanocoating\": {\"id\": 7}}}]}')",
        "insert into skinr_listings (id, skinr_id, seller_id, state, price_plex, quantity,
            created, expires, last_modified) values
            ('l1', '573b75f1393b5fb651dedda179977ab630d472641c86a3a1ba974b5a56b9d42b', 2112000010,
             'listed', 1250, 1, now() - interval '1 hour', now() + interval '1 day', now())",
        "insert into skinr_components (id, name, category, resource_file, color)
         values (7, 'Matte Rust', 'Material', 'res:/dx9/model/SpaceObjectFactory/materials/Rust_Matte.red', '#aa4422')",
    ] {
        sqlx::query(statement).execute(pool).await.unwrap();
    }
}

fn png(color: [u8; 4], size: u32) -> Vec<u8> {
    let image = RgbaImage::from_pixel(size, size, Rgba(color));
    let mut bytes = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
        .unwrap();
    bytes
}

/// A fresh directory with a thumbnail of the Rifter.
fn directory(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("skinr-og-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    let thumbnails = root.join("thumbnails");
    std::fs::create_dir_all(&thumbnails).unwrap();
    std::fs::write(
        thumbnails.join(format!("{RIFTER}-dark.png")),
        png(THUMBNAIL_COLOR, 64),
    )
    .unwrap();
    root
}

/// What the mock worker was asked for.
#[derive(Clone, Default)]
struct Worker {
    requests: Arc<std::sync::Mutex<Vec<String>>>,
    delay: Duration,
}

impl Worker {
    fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

async fn draw(worker: &Worker, request: String) -> Response {
    worker.requests.lock().unwrap().push(request);
    tokio::time::sleep(worker.delay).await;
    ([(header::CONTENT_TYPE, "image/png")], png(RENDER_COLOR, 32)).into_response()
}

async fn mock_worker(delay: Duration) -> (String, Worker) {
    let worker = Worker {
        delay,
        ..Worker::default()
    };
    let app = Router::new()
        .route(
            "/render",
            get(
                |State(worker): State<Worker>, RawQuery(query): RawQuery| async move {
                    draw(&worker, format!("/render?{}", query.unwrap_or_default())).await
                },
            ),
        )
        .route(
            "/render/skinr/{id}",
            get(
                |State(worker): State<Worker>,
                 Path(id): Path<String>,
                 RawQuery(query): RawQuery| async move {
                    draw(
                        &worker,
                        format!("/render/skinr/{id}?{}", query.unwrap_or_default()),
                    )
                    .await
                },
            ),
        )
        .with_state(worker.clone());
    (common::serve(app).await, worker)
}

/// The image server: portraits only, counting them.
async fn mock_images() -> (String, Arc<AtomicUsize>) {
    let portraits = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route(
            "/characters/{id}/portrait",
            get(|State(portraits): State<Arc<AtomicUsize>>| async move {
                portraits.fetch_add(1, Ordering::Relaxed);
                (
                    [(header::CONTENT_TYPE, "image/png")],
                    png([1, 2, 3, 255], 8),
                )
            }),
        )
        .with_state(Arc::clone(&portraits));
    (common::serve(app).await, portraits)
}

fn app(pool: PgPool, root: &std::path::Path, renderer: Option<String>, images: &str) -> Router {
    let unused = "http://127.0.0.1:1";
    router(AppState {
        pool,
        cdn: CdnClient::new(unused, unused),
        game_files: GameFiles::new(CdnClient::new(unused, unused), std::env::temp_dir()),
        thumbnails: root.join("thumbnails"),
        esi: EsiClient::new(unused, "skinr.space-test/1.0"),
        sso: None,
        og: Og::new(OgConfig {
            cache_dir: root.join("og"),
            thumbnails: root.join("thumbnails"),
            renderer_url: renderer,
            image_server: images.to_owned(),
            render_wait: Duration::from_millis(300),
        }),
        public_origin: "https://skinr.space".to_owned(),
        drawing: None,
        debug: false,
    })
}

struct Card {
    status: StatusCode,
    cache_control: String,
    body: Vec<u8>,
}

impl Card {
    fn is_final(&self) -> bool {
        self.cache_control.contains("max-age=3600")
    }

    /// The colour in the middle of the art.
    fn art(&self) -> [u8; 4] {
        let image = image::load_from_memory(&self.body).unwrap().to_rgba8();
        assert_eq!(image.dimensions(), (1200, 630));
        image.get_pixel(ART_PIXEL.0, ART_PIXEL.1).0
    }
}

async fn card(app: &Router, path: &str) -> Card {
    let response = common::get(app, path, None).await;
    let status = response.status();
    let cache_control = response
        .headers()
        .get(header::CACHE_CONTROL)
        .map(|value| value.to_str().unwrap().to_owned())
        .unwrap_or_default();
    if status == StatusCode::OK {
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
    }
    let body = response.into_body().collect().await.unwrap().to_bytes();
    Card {
        status,
        cache_control,
        body: body.to_vec(),
    }
}

#[tokio::test]
async fn a_ship_card_shows_its_thumbnail() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let root = directory("ship");
    let app = app(pool, &root, None, "http://127.0.0.1:1");

    let ship = card(&app, &format!("/og/ships/{RIFTER}.png")).await;
    assert_eq!(ship.status, StatusCode::OK);
    assert!(ship.is_final(), "{}", ship.cache_control);
    assert_eq!(ship.art(), THUMBNAIL_COLOR);

    for missing in ["/og/ships/1.png", "/og/ships/587", "/og/ships/rifter.png"] {
        assert_eq!(
            card(&app, missing).await.status,
            StatusCode::NOT_FOUND,
            "{missing}"
        );
    }
}

#[tokio::test]
async fn a_skin_card_shows_the_worker_render_and_keeps_it() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let root = directory("skin");
    let (renderer, worker) = mock_worker(Duration::ZERO).await;
    let app = app(pool, &root, Some(renderer), "http://127.0.0.1:1");
    let path = format!("/og/ships/{RIFTER}/skins/{KRUSUAL}.png");

    let skin = card(&app, &path).await;
    assert_eq!(skin.status, StatusCode::OK);
    assert!(skin.is_final(), "{}", skin.cache_control);
    assert_eq!(skin.art(), RENDER_COLOR);
    let asked = worker.requests();
    assert_eq!(asked.len(), 1);
    assert!(
        asked[0].starts_with(
            "/render?dna=mf4_t1%3Akrusual%3Aminmatar%3Apattern%3Fkrusual%3Bnone%3Bnone&size="
        ),
        "{}",
        asked[0]
    );

    // The card is kept, so the worker is not asked again.
    let again = card(&app, &path).await;
    assert_eq!(again.body, skin.body);
    assert_eq!(worker.count(), 1);

    assert_eq!(
        card(&app, &format!("/og/ships/{RIFTER}/skins/1.png"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

/// How long the slow mock worker takes over a render, in ms.
const SLOW_RENDER_MS: u64 = 5000;

#[tokio::test]
async fn a_slow_render_gets_a_stand_in_then_lands_for_the_next_request() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let root = directory("slow");
    // Far slower than the card waits (300 ms), so the second request still
    // finds it drawing on a busy machine.
    let (renderer, worker) = mock_worker(Duration::from_millis(SLOW_RENDER_MS)).await;
    let app = app(pool, &root, Some(renderer), "http://127.0.0.1:1");
    let path = format!("/og/ships/{RIFTER}/skins/{KRUSUAL}.png");

    let first = card(&app, &path).await;
    assert_eq!(first.status, StatusCode::OK);
    assert!(!first.is_final(), "{}", first.cache_control);
    assert_eq!(first.art(), THUMBNAIL_COLOR);

    // A second request while it draws joins the same render.
    assert!(!card(&app, &path).await.is_final());

    // Waits for the render to land.
    let mut later = card(&app, &path).await;
    for _ in 0..SLOW_RENDER_MS / 100 {
        if later.is_final() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        later = card(&app, &path).await;
    }
    assert!(later.is_final(), "{}", later.cache_control);
    assert_eq!(later.art(), RENDER_COLOR);
    assert_eq!(worker.count(), 1);
}

#[tokio::test]
async fn without_a_worker_a_card_still_renders() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let root = directory("down");
    let app = app(
        pool,
        &root,
        Some("http://127.0.0.1:1".to_owned()),
        "http://127.0.0.1:1",
    );

    let design = card(&app, &format!("/og/skinr/{DESIGN}.png")).await;
    assert_eq!(design.status, StatusCode::OK);
    assert!(!design.is_final());
    assert_eq!(design.art(), THUMBNAIL_COLOR);
}

#[tokio::test]
async fn a_design_card_asks_for_the_design_on_its_ship() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let root = directory("design");
    let (renderer, worker) = mock_worker(Duration::ZERO).await;
    let app = app(pool, &root, Some(renderer), "http://127.0.0.1:1");

    let design = card(&app, &format!("/og/skinr/{}.png", DESIGN.to_uppercase())).await;
    assert_eq!(design.status, StatusCode::OK);
    assert!(design.is_final());
    assert_eq!(design.art(), RENDER_COLOR);
    let asked = worker.requests();
    assert!(
        asked[0].starts_with(&format!("/render/skinr/{DESIGN}?ship={RIFTER}&size=")),
        "{}",
        asked[0]
    );

    for missing in [
        format!("/og/skinr/{}.png", "0".repeat(64)),
        "/og/skinr/573b75f1393b.png".to_owned(),
    ] {
        assert_eq!(card(&app, &missing).await.status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn a_designer_card_has_their_portrait_and_designs() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let root = directory("designer");
    let (renderer, worker) = mock_worker(Duration::ZERO).await;
    let (images, portraits) = mock_images().await;
    let app = app(pool, &root, Some(renderer), &images);

    let designer = card(&app, &format!("/og/designers/{ALICE}.png")).await;
    assert_eq!(designer.status, StatusCode::OK);
    assert!(designer.is_final(), "{}", designer.cache_control);
    assert_eq!(portraits.load(Ordering::Relaxed), 1);
    assert_eq!(worker.count(), 1);

    assert_eq!(
        card(&app, "/og/designers/1.png").await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn the_site_and_hub_cards_render() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let root = directory("site");
    let (renderer, _worker) = mock_worker(Duration::ZERO).await;
    let app = app(pool, &root, Some(renderer), "http://127.0.0.1:1");

    let home = card(&app, "/og/home.png").await;
    assert_eq!(home.status, StatusCode::OK);
    // Its ships have no thumbnails here.
    assert!(!home.is_final());
    let hub = card(&app, "/og/skinr.png").await;
    assert_eq!(hub.status, StatusCode::OK);
    assert!(hub.is_final(), "{}", hub.cache_control);
    assert_eq!(hub.art(), RENDER_COLOR);
}

#[tokio::test]
async fn the_sitemap_lists_every_page_with_the_frontend_urls() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    // A design that is not on sale stays out, and so does its designer.
    for statement in [
        "insert into esi_characters (id, name) values (2112000011, 'Bob')",
        "insert into skinr_designs (id, name, creator_id, ship_type_id, layout)
            values ('ffff', 'Hidden', 2112000011, 587, '{}')",
    ] {
        sqlx::query(statement).execute(&pool).await.unwrap();
    }
    let root = directory("sitemap");
    let app = app(pool, &root, None, "http://127.0.0.1:1");

    let response = common::get(&app, "/sitemap.xml", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("application/xml")
    );
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let xml = String::from_utf8(body.to_vec()).unwrap();
    let locations: Vec<&str> = xml
        .split("<loc>")
        .skip(1)
        .map(|rest| rest.split_once("</loc>").unwrap().0)
        .collect();
    assert_eq!(
        locations,
        vec![
            "https://skinr.space/",
            "https://skinr.space/skinr",
            "https://skinr.space/ships/rifter",
            "https://skinr.space/ships/rifter/skins/rifter-krusual-skin",
            "https://skinr.space/ships/rifter/skinr/rust-bucket-573b75f1393b",
            "https://skinr.space/skinr/designer/alice-oneil-2112000010",
        ]
    );
}
