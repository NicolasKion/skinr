//! Postgres access: connection pool, migrations and the seeded SDE build.

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// The local docker-compose Postgres (see `docker-compose.yml`).
pub const DEFAULT_DATABASE_URL: &str = "postgres://shipviewer:shipviewer@127.0.0.1:5434/shipviewer";

/// Enough for the request handlers and the importer side by side.
const MAX_POOL_CONNECTIONS: u32 = 10;

/// The `sde_meta` key recording the seeded SDE build number.
const SDE_BUILD_KEY: &str = "seeded_build";

pub fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_owned())
}

pub async fn connect() -> sqlx::Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(MAX_POOL_CONNECTIONS)
        .connect(&database_url())
        .await
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!().run(pool).await
}

/// The SDE build the database was last seeded from, if any.
pub async fn seeded_sde_build(pool: &PgPool) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("select value from sde_meta where key = $1")
        .bind(SDE_BUILD_KEY)
        .fetch_optional(pool)
        .await
}

/// Records the seeded SDE build, so unchanged bootstraps can skip.
pub async fn record_sde_build(pool: &PgPool, build: &str) -> sqlx::Result<()> {
    sqlx::query(
        "insert into sde_meta (key, value) values ($1, $2)
         on conflict (key) do update set value = excluded.value",
    )
    .bind(SDE_BUILD_KEY)
    .bind(build)
    .execute(pool)
    .await?;

    Ok(())
}
