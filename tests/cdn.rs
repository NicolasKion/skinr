//! The /cdn passthrough, driven through the real router against a local mock
//! of CCP's hosts on an ephemeral port.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::routing::get;
use http_body_util::BodyExt;
use ship_viewer::cdn::CdnClient;
use ship_viewer::server::{AppState, router};
use tower::ServiceExt;

/// Serves a fixed build pointer and one resource file, 403 for anything else
/// (S3's answer to a missing key).
async fn mock_upstream() -> String {
    let app = Router::new()
        .route(
            "/eveclient_TQ.json",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "application/json")],
                    r#"{"build": "1"}"#,
                )
            }),
        )
        .route(
            "/ab/abcdef_0123",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "application/octet-stream")],
                    "bytes",
                )
            }),
        )
        .fallback(|| async { StatusCode::FORBIDDEN });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

async fn app() -> Router {
    let upstream = mock_upstream().await;
    router(AppState {
        // Never connected: these routes do not touch the database.
        pool: sqlx::PgPool::connect_lazy("postgres://unused@127.0.0.1:1/unused").unwrap(),
        cdn: CdnClient::new(&upstream, &upstream),
    })
}

async fn get_path(app: Router, path: &str) -> (StatusCode, Option<String>, Option<String>, String) {
    let response = app
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let header_value = |name: header::HeaderName| {
        response
            .headers()
            .get(name)
            .map(|value| value.to_str().unwrap().to_owned())
    };
    let status = response.status();
    let content_type = header_value(header::CONTENT_TYPE);
    let cache_control = header_value(header::CACHE_CONTROL);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        content_type,
        cache_control,
        String::from_utf8_lossy(&body).into_owned(),
    )
}

#[tokio::test]
async fn the_build_pointer_is_cached_briefly() {
    let (status, content_type, cache_control, body) =
        get_path(app().await, "/cdn/binaries/eveclient_TQ.json").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_eq!(cache_control.as_deref(), Some("public, max-age=60"));
    assert_eq!(body, r#"{"build": "1"}"#);
}

#[tokio::test]
async fn resources_are_immutable() {
    let (status, content_type, cache_control, body) =
        get_path(app().await, "/cdn/resources/ab/abcdef_0123").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type.as_deref(), Some("application/octet-stream"));
    assert_eq!(
        cache_control.as_deref(),
        Some("public, max-age=31536000, immutable")
    );
    assert_eq!(body, "bytes");
}

#[tokio::test]
async fn a_missing_upstream_file_is_a_404() {
    let (status, _, cache_control, body) = get_path(app().await, "/cdn/resources/ab/missing").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(cache_control, None);
    assert_eq!(body, "");
}

#[tokio::test]
async fn unsafe_paths_never_reach_upstream() {
    let (status, _, _, _) = get_path(app().await, "/cdn/binaries/a/../eveclient_TQ.json").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unknown_paths_answer_json() {
    let (status, content_type, _, body) = get_path(app().await, "/api/nope").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_eq!(body, r#"{"message":"Not found."}"#);
}
