//! Sessions and the SSO tokens behind them, in Postgres.
//!
//! The browser holds a random session token in an HttpOnly cookie; the
//! table holds only its SHA-256. The SSO tokens stay in the API: an access
//! token is refreshed here shortly before it expires, and a refresh token
//! the SSO refuses ends the character's sessions.

use axum::http::HeaderMap;
use axum::http::header::COOKIE;
use rand::RngExt;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use super::sso::{self, SsoClient, Tokens};

pub const SESSION_COOKIE: &str = "skinr_session";
/// Carries the OAuth state and where to go afterwards through the SSO round trip.
pub const LOGIN_COOKIE: &str = "skinr_login";
/// Sessions last a month.
const SESSION_DAYS: i64 = 30;
/// The login cookie only has to survive the trip to the SSO and back.
const LOGIN_SECONDS: i64 = 10 * 60;
/// An access token this close to expiring is refreshed before use.
const REFRESH_MARGIN_SECONDS: i64 = 60;
/// Random bytes in a session token or OAuth state.
const TOKEN_BYTES: usize = 32;

/// 32 random bytes, hex encoded.
pub fn random_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    rand::rng().fill(&mut bytes[..]);
    hex(&bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The stored form of a session token.
fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

/// A cookie of the request. HTTP/2 may split cookies over several headers.
pub fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|cookies| cookies.split(';'))
        .find_map(|pair| {
            let (key, value) = pair.trim().split_once('=')?;
            (key == name).then(|| value.to_owned())
        })
}

/// A `Set-Cookie` value: HttpOnly, SameSite=Lax (it must ride along the
/// SSO's redirect back), and Secure when the site is served over HTTPS.
pub fn set_cookie(name: &str, value: &str, max_age: i64, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}")
}

pub fn session_cookie(token: &str, secure: bool) -> String {
    set_cookie(SESSION_COOKIE, token, SESSION_DAYS * 24 * 60 * 60, secure)
}

pub fn login_cookie(state: &str, return_to: &str, secure: bool) -> String {
    set_cookie(
        LOGIN_COOKIE,
        &format!("{state}~{return_to}"),
        LOGIN_SECONDS,
        secure,
    )
}

pub fn clear_cookie(name: &str, secure: bool) -> String {
    set_cookie(name, "", 0, secure)
}

/// The state and return path a login cookie carries.
pub fn parse_login_cookie(value: &str) -> Option<(&str, &str)> {
    value.split_once('~')
}

/// Where a login may send the browser afterwards: a path on this site, in
/// characters that need no escaping in a cookie. Anything else goes home.
pub fn safe_return_path(path: Option<&str>) -> &str {
    match path {
        Some(path)
            if path.starts_with('/')
                && !path.starts_with("//")
                && path
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"/-_.?=&%".contains(&byte)) =>
        {
            path
        }
        _ => "/",
    }
}

/// The logged-in character of a session.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionCharacter {
    pub id: i64,
    pub name: String,
    pub scopes: Vec<String>,
}

/// Stores a character's tokens after a login and opens a session for it;
/// returns the session token for the cookie. A changed owner hash means the
/// character was sold, so the previous owner's sessions end.
pub async fn log_in(
    pool: &PgPool,
    character: &sso::Character,
    tokens: &Tokens,
) -> sqlx::Result<String> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "delete from sessions where character_id = $1 and character_id in
            (select id from sso_characters where id = $1 and owner_hash <> $2)",
    )
    .bind(character.id)
    .bind(&character.owner_hash)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "insert into sso_characters (id, name, owner_hash, scopes, access_token, refresh_token,
            access_expires_at)
         values ($1, $2, $3, $4, $5, $6, now() + make_interval(secs => $7))
         on conflict (id) do update set name = excluded.name, owner_hash = excluded.owner_hash,
            scopes = excluded.scopes, access_token = excluded.access_token,
            refresh_token = excluded.refresh_token,
            access_expires_at = excluded.access_expires_at, updated_at = now()",
    )
    .bind(character.id)
    .bind(&character.name)
    .bind(&character.owner_hash)
    .bind(&character.scopes)
    .bind(&tokens.access_token)
    .bind(&tokens.refresh_token)
    .bind(tokens.expires_in as f64)
    .execute(&mut *tx)
    .await?;
    // Every login sweeps the sessions that ran out.
    sqlx::query("delete from sessions where expires_at < now()")
        .execute(&mut *tx)
        .await?;
    let token = random_token();
    sqlx::query(
        "insert into sessions (token_hash, character_id, expires_at)
         values ($1, $2, now() + make_interval(days => $3))",
    )
    .bind(token_hash(&token))
    .bind(character.id)
    .bind(SESSION_DAYS as i32)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(token)
}

/// The character of the request's session cookie, if the session is live.
pub async fn current(pool: &PgPool, headers: &HeaderMap) -> sqlx::Result<Option<SessionCharacter>> {
    let Some(token) = cookie(headers, SESSION_COOKIE) else {
        return Ok(None);
    };
    sqlx::query_as(
        "select c.id, c.name, c.scopes from sessions s
         join sso_characters c on c.id = s.character_id
         where s.token_hash = $1 and s.expires_at > now()",
    )
    .bind(token_hash(&token))
    .fetch_optional(pool)
    .await
}

/// Ends the request's session. When it was the character's last one, their
/// tokens go too; the refresh token is returned so it can be revoked.
pub async fn log_out(pool: &PgPool, headers: &HeaderMap) -> sqlx::Result<Option<(i64, String)>> {
    let Some(token) = cookie(headers, SESSION_COOKIE) else {
        return Ok(None);
    };
    let mut tx = pool.begin().await?;
    let character_id: Option<i64> =
        sqlx::query_scalar("delete from sessions where token_hash = $1 returning character_id")
            .bind(token_hash(&token))
            .fetch_optional(&mut *tx)
            .await?;
    let Some(character_id) = character_id else {
        return Ok(None);
    };
    let dropped: Option<String> = sqlx::query_scalar(
        "delete from sso_characters where id = $1
            and not exists (select 1 from sessions where character_id = $1)
         returning refresh_token",
    )
    .bind(character_id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(dropped.map(|refresh_token| (character_id, refresh_token)))
}

#[derive(Debug)]
pub enum TokenError {
    /// The SSO refused the refresh token; the character must log in again.
    Revoked,
    Sso(sso::Error),
    Db(sqlx::Error),
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenError::Revoked => write!(f, "the SSO refused the refresh token"),
            TokenError::Sso(error) => error.fmt(f),
            TokenError::Db(error) => write!(f, "token storage: {error}"),
        }
    }
}

impl std::error::Error for TokenError {}

impl From<sqlx::Error> for TokenError {
    fn from(error: sqlx::Error) -> Self {
        TokenError::Db(error)
    }
}

#[derive(sqlx::FromRow)]
struct StoredToken {
    access_token: String,
    refresh_token: String,
    expiring: bool,
}

/// A usable access token for the character, refreshed first when it is
/// about to expire. The row stays locked during a refresh, so parallel
/// requests never spend the same refresh token twice.
pub async fn access_token(
    pool: &PgPool,
    sso: &SsoClient,
    character_id: i64,
) -> Result<String, TokenError> {
    let mut tx = pool.begin().await?;
    let stored: Option<StoredToken> = sqlx::query_as(
        "select access_token, refresh_token,
                access_expires_at < now() + make_interval(secs => $2) as expiring
         from sso_characters where id = $1 for update",
    )
    .bind(character_id)
    .bind(REFRESH_MARGIN_SECONDS as f64)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(stored) = stored else {
        return Err(TokenError::Revoked);
    };
    if !stored.expiring {
        return Ok(stored.access_token);
    }

    let refreshed = match sso.refresh(&stored.refresh_token).await {
        Ok(tokens) => tokens,
        Err(error) if error.is_refused() => {
            tracing::info!(character_id, "the SSO refused a refresh token, logging out");
            sqlx::query("delete from sso_characters where id = $1")
                .bind(character_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            return Err(TokenError::Revoked);
        }
        Err(error) => return Err(TokenError::Sso(error)),
    };
    let character = sso
        .verify(&refreshed.access_token)
        .await
        .map_err(TokenError::Sso)?;
    if character.id != character_id {
        return Err(TokenError::Sso(sso::Error::Rejected(
            "a refresh returned another character",
        )));
    }
    sqlx::query(
        "update sso_characters set access_token = $2, refresh_token = $3, scopes = $4,
            access_expires_at = now() + make_interval(secs => $5), updated_at = now()
         where id = $1",
    )
    .bind(character_id)
    .bind(&refreshed.access_token)
    .bind(&refreshed.refresh_token)
    .bind(&character.scopes)
    .bind(refreshed.expires_in as f64)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(refreshed.access_token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookies_are_found_across_headers() {
        let mut headers = HeaderMap::new();
        headers.append(COOKIE, "theme=dark".parse().unwrap());
        headers.append(COOKIE, "a=1; skinr_session=abc".parse().unwrap());
        assert_eq!(cookie(&headers, SESSION_COOKIE).as_deref(), Some("abc"));
        assert_eq!(cookie(&headers, "missing"), None);
    }

    #[test]
    fn only_local_paths_are_returned_to() {
        assert_eq!(safe_return_path(Some("/me?skinr=abc")), "/me?skinr=abc");
        assert_eq!(safe_return_path(Some("//evil.example")), "/");
        assert_eq!(safe_return_path(Some("https://evil.example")), "/");
        assert_eq!(safe_return_path(Some("/a;b")), "/");
        assert_eq!(safe_return_path(None), "/");
    }

    #[test]
    fn the_login_cookie_round_trips() {
        let cookie = login_cookie("state", "/me", true);
        assert!(cookie.contains("HttpOnly") && cookie.ends_with("; Secure"));
        let value = cookie.split(';').next().unwrap().split_once('=').unwrap().1;
        assert_eq!(parse_login_cookie(value), Some(("state", "/me")));
    }

    #[test]
    fn sessions_are_stored_hashed() {
        let token = random_token();
        assert_eq!(token.len(), TOKEN_BYTES * 2);
        assert_ne!(token_hash(&token), token);
        assert_eq!(token_hash(&token), token_hash(&token));
    }
}
