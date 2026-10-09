#[tokio::main]
async fn main() {
    // Structured logs; tune with RUST_LOG (e.g. RUST_LOG=ship_viewer=debug).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Local configuration from .env, if present; real environment wins.
    dotenvy::dotenv().ok();

    let addr = ship_viewer::server::bind_addr();

    let pool = ship_viewer::db::connect()
        .await
        .expect("database connection");
    ship_viewer::db::migrate(&pool)
        .await
        .expect("database migrations");

    let app = ship_viewer::server::router(ship_viewer::server::AppState {
        pool,
        cdn: ship_viewer::cdn::CdnClient::from_env(),
    });

    tracing::info!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind address");
    axum::serve(listener, app).await.expect("serve");
}
