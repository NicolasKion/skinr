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

    // Keeps the Paragon Hub's SKINR designs current; ESI_SYNC=0 turns it off.
    if std::env::var("ESI_SYNC").map_or(true, |value| value != "0") {
        skinr::esi::skinr::spawn(pool.clone(), skinr::esi::client::EsiClient::from_env());
    }

    let app = skinr::server::router(skinr::server::AppState {
        pool,
        cdn: skinr::cdn::CdnClient::from_env(),
        thumbnails: skinr::server::thumbnails_dir(),
    });

    tracing::info!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind address");
    axum::serve(listener, app).await.expect("serve");
}
