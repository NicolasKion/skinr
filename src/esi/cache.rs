//! ESI answers kept in Postgres (`esi_cache`) until they expire.
//!
//! ESI is a cache itself: asking again before an answer expires at best
//! returns the same copy and at worst reads as cache circumvention. So a
//! single answer is kept for its `Cache-Control: max-age` and then asked for
//! again with its ETag, which ESI answers with a cheap 304 when nothing
//! changed. Cursor-paginated lists have no single ETag; their pages are
//! walked once and the whole list is kept for a fixed time.

use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::PgPool;

use super::client::{Error as EsiError, EsiClient};

/// How many records to ask a cursor-paginated route for per page (ESI allows 10 to 100).
const PAGE_SIZE: u32 = 100;
/// The cursor value that means "from the beginning".
const FIRST_CURSOR: &str = "0";
/// Pages walked for one list at most, so one request stays inside a route's bucket.
const MAX_PAGES: usize = 10;

#[derive(Debug)]
pub enum Error {
    Esi(EsiError),
    Db(sqlx::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Esi(error) => error.fmt(f),
            Error::Db(error) => write!(f, "ESI cache: {error}"),
            Error::Json(error) => write!(f, "ESI answered unexpected JSON: {error}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<EsiError> for Error {
    fn from(error: EsiError) -> Self {
        Error::Esi(error)
    }
}

impl From<sqlx::Error> for Error {
    fn from(error: sqlx::Error) -> Self {
        Error::Db(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Json(error)
    }
}

#[derive(sqlx::FromRow)]
struct Entry {
    body: String,
    etag: Option<String>,
    fresh: bool,
}

async fn entry(pool: &PgPool, key: &str) -> sqlx::Result<Option<Entry>> {
    sqlx::query_as("select body, etag, expires_at > now() as fresh from esi_cache where key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await
}

async fn store(
    pool: &PgPool,
    key: &str,
    body: &str,
    etag: Option<&str>,
    keep: Duration,
) -> sqlx::Result<()> {
    sqlx::query(
        "insert into esi_cache (key, body, etag, expires_at)
         values ($1, $2, $3, now() + make_interval(secs => $4))
         on conflict (key) do update set body = excluded.body, etag = excluded.etag,
            expires_at = excluded.expires_at",
    )
    .bind(key)
    .bind(body)
    .bind(etag)
    .bind(keep.as_secs_f64())
    .execute(pool)
    .await?;
    Ok(())
}

/// One ESI answer, from the cache while it is fresh. `floor` is the shortest
/// time an answer is kept, for routes that do not say.
pub async fn get<T: DeserializeOwned>(
    pool: &PgPool,
    esi: &EsiClient,
    key: &str,
    path: &str,
    bearer: Option<&str>,
    floor: Duration,
) -> Result<T, Error> {
    let cached = entry(pool, key).await?;
    if let Some(cached) = &cached
        && cached.fresh
    {
        return Ok(serde_json::from_str(&cached.body)?);
    }

    let etag = cached.as_ref().and_then(|cached| cached.etag.as_deref());
    let fetched = esi.fetch(path, bearer, etag).await?;
    let keep = fetched.max_age.unwrap_or(floor).max(floor);
    let body = match (fetched.body, cached) {
        (Some(body), _) => body,
        // A 304: the copy we hold is current again.
        (None, Some(cached)) => cached.body,
        (None, None) => {
            return Err(Error::Esi(EsiError::Status(
                reqwest::StatusCode::NOT_MODIFIED,
                "a 304 without a cached copy".to_owned(),
            )));
        }
    };
    let value = serde_json::from_str(&body)?;
    store(pool, key, &body, fetched.etag.as_deref(), keep).await?;
    Ok(value)
}

#[derive(serde::Deserialize)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
struct Page<T> {
    #[serde(default)]
    cursor: Option<Cursor>,
    #[serde(default)]
    listings: Vec<T>,
}

#[derive(serde::Deserialize)]
struct Cursor {
    #[serde(default)]
    after: Option<String>,
}

/// Every record of a cursor-paginated listings route (`path` without its
/// query), from the cache while it is fresh. The walk goes forward from the
/// beginning until a page comes back empty.
pub async fn listings<T: DeserializeOwned + Serialize>(
    pool: &PgPool,
    esi: &EsiClient,
    key: &str,
    path: &str,
    bearer: Option<&str>,
    keep: Duration,
) -> Result<Vec<T>, Error> {
    if let Some(cached) = entry(pool, key).await?
        && cached.fresh
    {
        return Ok(serde_json::from_str(&cached.body)?);
    }

    let mut all = Vec::new();
    let mut cursor = FIRST_CURSOR.to_owned();
    for _ in 0..MAX_PAGES {
        let fetched = esi
            .fetch(
                &format!("{path}?after={cursor}&limit={PAGE_SIZE}"),
                bearer,
                None,
            )
            .await?;
        let page: Page<T> = serde_json::from_str(fetched.body.as_deref().unwrap_or("{}"))?;
        if page.listings.is_empty() {
            break;
        }
        all.extend(page.listings);
        match page.cursor.and_then(|cursor| cursor.after) {
            Some(next) => cursor = next,
            None => break,
        }
    }
    store(pool, key, &serde_json::to_string(&all)?, None, keep).await?;
    Ok(all)
}

/// Drops every cached answer whose key starts with `prefix` (a character's,
/// when they log out).
pub async fn forget(pool: &PgPool, prefix: &str) -> sqlx::Result<()> {
    sqlx::query("delete from esi_cache where starts_with(key, $1)")
        .bind(prefix)
        .execute(pool)
        .await?;
    Ok(())
}
