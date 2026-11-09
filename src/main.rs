#[tokio::main]
async fn main() {
    // Structured logs; tune with RUST_LOG (e.g. RUST_LOG=skinr=debug).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Local configuration from .env, if present; real environment wins.
    dotenvy::dotenv().ok();

    let addr = skinr::server::bind_addr();

    let pool = skinr::db::connect().await.expect("database connection");
    skinr::db::migrate(&pool)
        .await
        .expect("database migrations");

    let esi = skinr::esi::client::EsiClient::from_env();
    // Keeps the Paragon Hub's SKINR designs current; ESI_SYNC=0 turns it off.
    if std::env::var("ESI_SYNC").map_or(true, |value| value != "0") {
        skinr::esi::skinr::spawn(pool.clone(), esi.clone());
    }

    let sso = skinr::auth::sso::SsoClient::from_env();
    if sso.is_none() {
        tracing::info!("EVE_CLIENT_ID, EVE_CLIENT_SECRET or EVE_CALLBACK_URL unset: login is off");
    }

    let cdn = skinr::cdn::CdnClient::from_env();
    let game_files = skinr::game_files::GameFiles::new(cdn.clone(), skinr::game_files::cache_dir());
    game_files.warm();

    let thumbnails = skinr::server::thumbnails_dir();
    let og_config = skinr::og::OgConfig::from_env(thumbnails.clone());
    // As many drawings in flight as the render worker has processes (its WORKERS).
    let concurrency = std::env::var("THUMBNAIL_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    let drawing = og_config.renderer_url.as_deref().map(|renderer| {
        skinr::thumbnails::Thumbnails::new(pool.clone(), thumbnails.clone(), renderer)
            .with_concurrency(concurrency)
    });
    match &drawing {
        // Keeps every ship's thumbnails current; THUMBNAIL_SYNC=0 turns it off.
        Some(drawing) if std::env::var("THUMBNAIL_SYNC").map_or(true, |value| value != "0") => {
            skinr::thumbnails::spawn(drawing.clone());
        }
        Some(_) => {}
        None => tracing::info!(
            "RENDERER_URL unset: thumbnails are not drawn, cards show them instead of renders"
        ),
    }
    // Tools that start renders on request, under /api/debug; off in production.
    let debug = std::env::var("DEBUG_TOOLS").is_ok_and(|value| value == "1");
    if debug {
        tracing::info!("DEBUG_TOOLS=1: debug tools under /api/debug");
    }
    let app = skinr::server::router(skinr::server::AppState {
        pool,
        cdn,
        game_files,
        thumbnails,
        esi,
        sso,
        og: skinr::og::Og::new(og_config),
        public_origin: skinr::server::sitemap::public_origin(),
        drawing,
        debug,
    });

    tracing::info!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind address");
    axum::serve(listener, app).await.expect("serve");
}
