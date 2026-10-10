//! /api/thumbnails, driven through the real router against a temporary directory.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use skinr::cdn::CdnClient;
use skinr::server::{AppState, router};
use tower::ServiceExt;

/// A thumbnail directory holding one ship's dark thumbnail, and a file beside
/// it that must stay out of reach.
fn directory(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("skinr-thumbnails-{name}-{}", std::process::id()));
    let thumbnails = root.join("thumbnails");
    std::fs::create_dir_all(&thumbnails).unwrap();
    std::fs::write(thumbnails.join("17715-dark.png"), b"png bytes").unwrap();
    std::fs::write(root.join("secret.txt"), b"secret").unwrap();
    thumbnails
}

async fn get(thumbnails: std::path::PathBuf, path: &str) -> (StatusCode, Option<String>, Vec<u8>) {
    let app = router(AppState {
        // Never connected: thumbnails do not touch the database.
        pool: sqlx::PgPool::connect_lazy("postgres://unused@127.0.0.1:1/unused").unwrap(),
        cdn: CdnClient::new("http://127.0.0.1:1", "http://127.0.0.1:1"),
        thumbnails,
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
