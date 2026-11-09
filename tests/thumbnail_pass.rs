//! Keeping ship thumbnails current (`skinr::thumbnails`): a pass over a seeded
//! database against a mock render worker on an ephemeral port, writing into a
//! temporary directory. Needs Postgres (`docker compose up -d postgres`).

mod common;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Value, json};
use skinr::thumbnails::{PassError, Report, Thumbnails};
use sqlx::PgPool;

const RIFTER: i32 = 587;
const SLASHER: i32 = 585;
const RIFTER_DNA: &str = "mf4_t1:minmatarbase:minmatar";
/// A DNA the mock worker cannot draw.
const BROKEN_HULL: &str = "broken_t1";

async fn seed(pool: &PgPool) {
    for statement in [
        "insert into ship_groups (id, name) values (25, 'Frigate')",
        "insert into ships (id, name, group_id, sof_hull, sof_faction, sof_race)
         values (587, 'Rifter', 25, 'mf4_t1', 'minmatarbase', 'minmatar'),
                (585, 'Slasher', 25, 'mf2_t1', 'minmatarbase', 'minmatar')",
    ] {
        sqlx::query(statement).execute(pool).await.unwrap();
    }
}

/// A tiny real PNG.
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

/// What the mock worker was asked for, in order: a DNA, or `design <id> on <ship>`.
type Asked = Arc<Mutex<Vec<String>>>;

async fn thumbnail(State(asked): State<Asked>, Json(request): Json<Value>) -> (StatusCode, String) {
    let dna = match request["dna"].as_str() {
        Some(dna) => dna.to_owned(),
        None => format!(
            "design {} on {}",
            request["design"].as_str().unwrap(),
            request["ship"]
        ),
    };
    assert_eq!(request["size"], 512);
    asked.lock().unwrap().push(dna.clone());
    if dna.starts_with(BROKEN_HULL) {
        return (StatusCode::UNPROCESSABLE_ENTITY, "nothing was drawn".into());
    }
    let encoded = BASE64.encode(png());
    let answer = json!({
        "pngs": {"dark": encoded, "light": encoded},
        "used": [format!("res:/dx9/model/ship/{dna}.gr2")],
        "missing": [],
    });
    (StatusCode::OK, answer.to_string())
}

/// A mock worker; returns its base URL and what it was asked.
async fn worker() -> (String, Asked) {
    let asked = Asked::default();
    let app = Router::new()
        .route("/thumbnail", post(thumbnail))
        .with_state(asked.clone());
    (common::serve(app).await, asked)
}

fn directory(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "skinr-thumbnail-pass-{name}-{}",
        std::process::id()
    ));
    std::fs::remove_dir_all(&dir).ok();
    dir
}

fn read(dir: &std::path::Path, file: &str) -> Option<Vec<u8>> {
    std::fs::read(dir.join(file)).ok()
}

#[tokio::test]
async fn a_pass_draws_the_ships_without_thumbnails_once() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let (base, asked) = worker().await;
    let dir = directory("once");
    let thumbnails = Thumbnails::new(pool, dir.clone(), &format!("{base}/"));

    let report = thumbnails.pass().await.unwrap();
    assert_eq!(
        report,
        Report {
            checked: 2,
            drawn: 2,
            failed: 0
        }
    );
    for theme in ["dark", "light"] {
        assert_eq!(read(&dir, &format!("{RIFTER}-{theme}.png")), Some(png()));
    }
    assert_eq!(
        read(&dir, &format!("{RIFTER}.dna")).unwrap(),
        RIFTER_DNA.as_bytes()
    );
    let files: Vec<String> =
        serde_json::from_slice(&read(&dir, &format!("{RIFTER}.files")).unwrap()).unwrap();
    assert_eq!(files, [format!("res:/dx9/model/ship/{RIFTER_DNA}.gr2")]);
    assert!(read(&dir, &format!("{SLASHER}.dna")).is_some());
    // Nothing is left half written.
    let parts = std::fs::read_dir(&dir)
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".part")
        })
        .count();
    assert_eq!(parts, 0);

    // Everything is current now: the next pass asks for nothing.
    asked.lock().unwrap().clear();
    let report = thumbnails.pass().await.unwrap();
    assert_eq!(report.drawn, 0);
    assert!(asked.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_changed_or_incomplete_ship_is_drawn_again() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let (base, asked) = worker().await;
    let dir = directory("changed");
    let thumbnails = Thumbnails::new(pool.clone(), dir.clone(), &base);
    thumbnails.pass().await.unwrap();
    asked.lock().unwrap().clear();

    // An SDE import gave the Rifter a material set; the Slasher lost a file.
    sqlx::query("update ships set sof_commands = 'material?a;b;c;d' where id = $1")
        .bind(RIFTER)
        .execute(&pool)
        .await
        .unwrap();
    std::fs::remove_file(dir.join(format!("{SLASHER}-light.png"))).unwrap();

    let report = thumbnails.pass().await.unwrap();
    assert_eq!(report.drawn, 2);
    let mut asked = asked.lock().unwrap().clone();
    asked.sort();
    assert_eq!(
        asked,
        [
            "mf2_t1:minmatarbase:minmatar".to_owned(),
            format!("{RIFTER_DNA}:material?a;b;c;d"),
        ]
    );
    assert_eq!(
        read(&dir, &format!("{RIFTER}.dna")).unwrap(),
        format!("{RIFTER_DNA}:material?a;b;c;d").as_bytes()
    );
    assert!(read(&dir, &format!("{SLASHER}-light.png")).is_some());
}

#[tokio::test]
async fn a_ship_the_worker_cannot_draw_is_skipped_and_tried_again() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    sqlx::query("update ships set sof_hull = $1 where id = $2")
        .bind(BROKEN_HULL)
        .bind(SLASHER)
        .execute(&pool)
        .await
        .unwrap();
    let (base, asked) = worker().await;
    let dir = directory("broken");
    let thumbnails = Thumbnails::new(pool, dir.clone(), &base);

    let report = thumbnails.pass().await.unwrap();
    assert_eq!((report.drawn, report.failed), (1, 1));
    assert!(read(&dir, &format!("{RIFTER}.dna")).is_some());
    for file in ["dark.png", "light.png"].map(|end| format!("{SLASHER}-{end}")) {
        assert!(read(&dir, &file).is_none());
    }
    assert!(read(&dir, &format!("{SLASHER}.dna")).is_none());

    // The next pass asks for it again, and only for it.
    asked.lock().unwrap().clear();
    let report = thumbnails.pass().await.unwrap();
    assert_eq!((report.drawn, report.failed), (0, 1));
    assert_eq!(asked.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn an_unreachable_worker_stops_the_pass() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let dir = directory("unreachable");
    // Nothing listens on port 1.
    let thumbnails = Thumbnails::new(pool, dir.clone(), "http://127.0.0.1:1");

    let error = thumbnails.pass().await.unwrap_err();
    assert!(matches!(error, PassError::Unreachable(_)), "{error}");
    assert!(!dir.exists() || std::fs::read_dir(&dir).unwrap().next().is_none());
}

#[tokio::test]
async fn a_pass_draws_every_skin_and_design_after_the_ships() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let design = "ab".repeat(32);
    for statement in [
        // One skin on both hulls, and a design for the Rifter.
        "insert into ship_skins (ship_id, skin_id, name, sof_faction, sof_commands)
         values (587, 1001, 'Glory', 'minmatarbase', 'pattern?glory;a;b'),
                (585, 1001, 'Glory', 'minmatarbase', 'pattern?glory;a;b')",
        "insert into esi_characters (id, name) values (2112000010, 'Alice')",
    ] {
        sqlx::query(statement).execute(&pool).await.unwrap();
    }
    sqlx::query(
        "insert into skinr_designs (id, name, creator_id, ship_type_id, layout)
         values ($1, 'Rust Bucket', 2112000010, 587, '{\"slots\": []}')",
    )
    .bind(&design)
    .execute(&pool)
    .await
    .unwrap();
    let (base, asked) = worker().await;
    let dir = directory("everything");
    let thumbnails = Thumbnails::new(pool, dir.clone(), &base);

    let report = thumbnails.pass().await.unwrap();
    assert_eq!((report.checked, report.drawn, report.failed), (5, 5, 0));
    let asked = asked.lock().unwrap().clone();
    assert_eq!(
        asked,
        [
            "mf2_t1:minmatarbase:minmatar".to_owned(),
            RIFTER_DNA.to_owned(),
            // Not on sale: after the skins.
            "mf2_t1:minmatarbase:minmatar:pattern?glory;a;b".to_owned(),
            format!("{RIFTER_DNA}:pattern?glory;a;b"),
            format!("design {design} on {RIFTER}"),
        ]
    );
    // A skin's files are named by hull and skin, a design's by its id, and
    // the design is marked with its look.
    assert!(read(&dir, &format!("skins/{SLASHER}_1001-dark.png")).is_some());
    assert!(read(&dir, &format!("skins/{RIFTER}_1001-light.png")).is_some());
    assert_eq!(
        read(&dir, &format!("designs/{design}.dna")).unwrap(),
        format!(r#"{RIFTER_DNA}|{{"slots": []}}"#).as_bytes()
    );
}

#[tokio::test]
async fn a_pass_keeps_as_many_drawings_in_flight_as_asked() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    /// Drawings in flight now, and the most there ever were.
    #[derive(Clone, Default)]
    struct Load(Arc<(AtomicUsize, AtomicUsize)>);

    async fn slow(State(load): State<Load>) -> String {
        let now = load.0.0.fetch_add(1, Ordering::SeqCst) + 1;
        load.0.1.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(150)).await;
        load.0.0.fetch_sub(1, Ordering::SeqCst);
        let encoded = BASE64.encode(png());
        json!({"pngs": {"dark": encoded, "light": encoded}, "used": [], "missing": []}).to_string()
    }

    let (pool, _db) = common::database().await;
    seed(&pool).await;
    sqlx::query(
        "insert into ship_skins (ship_id, skin_id, name, sof_faction)
         values (587, 1, 'A', 'x'), (587, 2, 'B', 'x'), (585, 3, 'C', 'x'), (585, 4, 'D', 'x')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let load = Load::default();
    let app = Router::new()
        .route("/thumbnail", post(slow))
        .with_state(load.clone());
    let base = common::serve(app).await;
    let thumbnails = Thumbnails::new(pool, directory("concurrent"), &base).with_concurrency(3);

    let report = thumbnails.pass().await.unwrap();
    assert_eq!(report.drawn, 6);
    assert_eq!(load.0.1.load(Ordering::SeqCst), 3);
}
