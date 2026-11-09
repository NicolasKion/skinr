//! /api/thumbnails and /api/ships/{id}/files (the render outputs), driven
//! through the real router against a temporary directory.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use skinr::cdn::CdnClient;
use skinr::game_files::GameFiles;
use skinr::server::{AppState, router};
use tower::ServiceExt;

/// A thumbnail directory holding one ship's dark thumbnail, and a file beside
/// it that must stay out of reach.
fn directory(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("skinr-thumbnails-{name}-{}", std::process::id()));
    let thumbnails = root.join("thumbnails");
    std::fs::create_dir_all(&thumbnails).unwrap();
    std::fs::write(thumbnails.join("17715-dark.png"), b"png bytes").unwrap();
    // A real (tiny, transparent) render, for the WebP copies.
    let mut png = Vec::new();
    image::RgbaImage::from_pixel(512, 512, image::Rgba([200, 30, 30, 128]))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    std::fs::write(thumbnails.join("603-light.png"), png).unwrap();
    // What the render of ship 603 used, one entry of which is no game path.
    std::fs::write(
        thumbnails.join("603.files"),
        r#"["res:/dx9/model/ship/a.gr2","res:/texture/b.dds","/etc/passwd"]"#,
    )
    .unwrap();
    std::fs::write(root.join("secret.txt"), b"secret").unwrap();
    thumbnails
}

async fn get(thumbnails: std::path::PathBuf, path: &str) -> (StatusCode, Option<String>, Vec<u8>) {
    let app = router(AppState {
        // Never connected: thumbnails do not touch the database.
        pool: sqlx::PgPool::connect_lazy("postgres://unused@127.0.0.1:1/unused").unwrap(),
        cdn: CdnClient::new("http://127.0.0.1:1", "http://127.0.0.1:1"),
        game_files: GameFiles::new(
            CdnClient::new("http://127.0.0.1:1", "http://127.0.0.1:1"),
            std::env::temp_dir(),
        ),
        thumbnails,
        esi: skinr::esi::client::EsiClient::new("http://127.0.0.1:1", "skinr.space-test/1.0"),
        sso: None,
        og: skinr::og::Og::new(skinr::og::OgConfig::from_env(std::env::temp_dir())),
        public_origin: "https://skinr.space".to_owned(),
        drawing: None,
        debug: false,
    });
    let response = app
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|value| value.to_str().unwrap().to_owned());
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, content_type, body.to_vec())
}

#[tokio::test]
async fn a_rendered_thumbnail_is_served_as_png() {
    let (status, content_type, body) =
        get(directory("served"), "/api/thumbnails/17715-dark.png").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type.as_deref(), Some("image/png"));
    assert_eq!(body, b"png bytes");
}

#[tokio::test]
async fn a_missing_thumbnail_is_not_found() {
    let (status, _, _) = get(directory("missing"), "/api/thumbnails/17715-light.png").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn only_thumbnail_names_are_read() {
    for path in [
        "/api/thumbnails/..%2Fsecret.txt",
        "/api/thumbnails/17715-sepia.png",
        "/api/thumbnails/abc-dark.png",
        "/api/thumbnails/17715-dark.txt",
    ] {
        let (status, _, body) = get(directory("names"), path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_ne!(body, b"secret", "{path}");
    }
}

#[tokio::test]
async fn a_card_size_webp_is_made_from_the_render() {
    let (status, content_type, body) =
        get(directory("webp"), "/api/thumbnails/603-light-256.webp").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type.as_deref(), Some("image/webp"));
    assert_eq!(&body[8..12], b"WEBP");
}

#[tokio::test]
async fn only_card_sizes_are_made() {
    for path in [
        "/api/thumbnails/603-light-300.webp",
        "/api/thumbnails/603-light.webp",
        "/api/thumbnails/604-light-256.webp",
    ] {
        let (status, _, _) = get(directory("sizes"), path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn a_ship_lists_the_game_files_its_render_used() {
    let (status, content_type, body) = get(directory("files"), "/api/ships/603/files").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_eq!(
        body,
        br#"["res:/dx9/model/ship/a.gr2","res:/texture/b.dds"]"#
    );
}

#[tokio::test]
async fn a_ship_not_rendered_yet_lists_nothing() {
    let (status, _, body) = get(directory("no-files"), "/api/ships/604/files").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"[]");
}
