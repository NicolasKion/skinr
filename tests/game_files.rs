//! /cdn/build, /cdn/res and /cdn/list, driven through the real router against
//! a local mock of CCP's hosts on an ephemeral port, with a temporary cache.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::routing::get;
use http_body_util::BodyExt;
use skinr::cdn::CdnClient;
use skinr::game_files::GameFiles;
use skinr::server::{AppState, router};
use tower::ServiceExt;

const BUILD: &str = "7";
/// Compresses well, so it gets a zstd copy.
const TEXT: &str = "ship ship ship ship ship ship ship ship ship ship ship ship ship ship";

/// CCP's hosts for build 7: the pointer, the app index, two resfileindexes
/// (the second overriding the first) and their files. Counts file fetches.
async fn mock_upstream(fetches: Arc<AtomicUsize>) -> String {
    let app = Router::new()
        .route("/eveclient_TQ.json", get(|| async { r#"{"build": "7"}"# }))
        .route(
            "/eveonline_7.txt",
            get(|| async {
                "app:/bin/x.exe,00/00_x,x,1,1,0\n\
                 app:/resfileindex.txt,0a/0a_main,m,1,1,0\n\
                 app:/resfileindex_prefetch.txt,0b/0b_pre,p,1,1,0\n"
            }),
        )
        .route(
            "/0a/0a_main",
            get(|| async {
                "res:/dx9/model/ship/hull.gr2,aa/aa_old,o,1,1\n\
                 res:/dx9/scene/space.black,bb/bb_scene,s,1,1\n\
                 res:/sof/materials/steel.black,cc/cc_steel,c,1,1\n\
                 res:/sof/materials/glass.black,cc/cc_glass,g,1,1\n"
            }),
        )
        .route(
            "/0b/0b_pre",
            get(|| async { "res:/dx9/model/ship/hull.gr2,aa/aa_text,t,1,1\n" }),
        )
        .route(
            "/aa/aa_text",
            get(move || {
                fetches.fetch_add(1, Ordering::SeqCst);
                async { TEXT }
            }),
        )
        .route("/bb/bb_scene", get(|| async { "scene" }))
        .fallback(|| async { StatusCode::FORBIDDEN });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

/// The app over the mock, with an empty cache of its own.
async fn app(name: &str) -> (Router, Arc<AtomicUsize>) {
    let fetches = Arc::new(AtomicUsize::new(0));
    let upstream = mock_upstream(fetches.clone()).await;
    let cache =
        std::env::temp_dir().join(format!("skinr-game-files-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cache);
    let cdn = CdnClient::new(&upstream, &upstream);
    let app = router(AppState {
        // Never connected: these routes do not touch the database.
        pool: sqlx::PgPool::connect_lazy("postgres://unused@127.0.0.1:1/unused").unwrap(),
        game_files: GameFiles::new(cdn.clone(), cache),
        cdn,
        thumbnails: std::env::temp_dir(),
        esi: skinr::esi::client::EsiClient::new("http://127.0.0.1:1", "skinr.space-test/1.0"),
        sso: None,
        og: skinr::og::Og::new(skinr::og::OgConfig::from_env(std::env::temp_dir())),
        public_origin: "https://skinr.space".to_owned(),
        drawing: None,
        debug: false,
    });
    (app, fetches)
}

async fn get_path(
    app: &Router,
    path: &str,
    encoding: Option<&str>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut request = Request::get(path);
    if let Some(encoding) = encoding {
        request = request.header(header::ACCEPT_ENCODING, encoding);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, body.to_vec())
}

#[tokio::test]
async fn the_build_is_current_and_short_lived() {
    let (app, _) = app("build").await;
    let (status, headers, body) = get_path(&app, "/cdn/build", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=60");
    assert_eq!(body, br#"{"build":"7"}"#);
}

#[tokio::test]
async fn a_file_is_served_by_path_from_the_last_index_that_has_it() {
    let (app, _) = app("served").await;
    let (status, headers, body) = get_path(
        &app,
        &format!("/cdn/res/{BUILD}/DX9/Model/Ship/Hull.GR2"),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, TEXT.as_bytes());
    assert!(headers.get(header::CONTENT_ENCODING).is_none());
    assert_eq!(
        headers[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
}

#[tokio::test]
async fn a_client_that_takes_zstd_gets_it() {
    let (app, _) = app("zstd").await;
    let (status, headers, body) = get_path(
        &app,
        &format!("/cdn/res/{BUILD}/dx9/model/ship/hull.gr2"),
        Some("gzip, br, zstd"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_ENCODING], "zstd");
    assert_eq!(headers[header::VARY], "accept-encoding");
    assert_eq!(zstd::decode_all(body.as_slice()).unwrap(), TEXT.as_bytes());
}

#[tokio::test]
async fn a_file_is_fetched_from_upstream_once() {
    let (app, fetches) = app("once").await;
    let path = format!("/cdn/res/{BUILD}/dx9/model/ship/hull.gr2");
    for encoding in [None, Some("zstd"), None, Some("zstd")] {
        let (status, _, _) = get_path(&app, &path, encoding).await;
        assert_eq!(status, StatusCode::OK);
    }

    assert_eq!(fetches.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_red_path_reads_its_black() {
    let (app, _) = app("red").await;
    let (status, _, body) =
        get_path(&app, &format!("/cdn/res/{BUILD}/dx9/scene/space.red"), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"scene");
}

#[tokio::test]
async fn unknown_files_builds_and_paths_are_not_found() {
    let (app, _) = app("missing").await;
    for path in [
        format!("/cdn/res/{BUILD}/dx9/nothing.dds"),
        "/cdn/res/6/dx9/model/ship/hull.gr2".to_owned(),
        format!("/cdn/res/{BUILD}/dx9/../dx9/model/ship/hull.gr2"),
        // Listed in the index, but the host does not have it.
        format!("/cdn/res/{BUILD}/sof/materials/steel.black"),
    ] {
        let (status, _, _) = get_path(&app, &path, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn a_folder_lists_its_files() {
    let (app, _) = app("list").await;
    let (status, headers, body) =
        get_path(&app, &format!("/cdn/list/{BUILD}/sof/materials"), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    assert_eq!(body, br#"["glass.black","steel.black"]"#);
}
