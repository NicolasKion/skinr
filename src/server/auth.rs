//! Log in with EVE: `/eve/login` sends the browser to the SSO,
//! `/eve/callback` takes it back, trades the code for tokens (here, with
//! the client secret) and opens a session; `/eve/logout` ends it.
//! `/api/me` says who is logged in.

use std::collections::HashMap;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{AppendHeaders, IntoResponse, Redirect, Response};
use serde::Serialize;

use super::AppState;
use super::api::{database_error, error};
use crate::auth::session::{self, LOGIN_COOKIE, SESSION_COOKIE};
use crate::auth::{LOGIN_SCOPES, sso::SsoClient};
use crate::esi::{cache, character};

/// Where a failed login lands; the page says it did not work.
const LOGIN_FAILED: &str = "/me?login=failed";

fn sso_or_off(state: &AppState) -> Result<&SsoClient, Response> {
    state.sso.as_ref().ok_or_else(|| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Logging in is not set up on this server.",
        )
    })
}

/// `/eve/login?return=/path`: off to the SSO, with a fresh state.
pub async fn login(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, Response> {
    let sso = sso_or_off(&state)?;
    let oauth_state = session::random_token();
    let return_to = session::safe_return_path(query.get("return").map(String::as_str));
    let url = sso
        .authorize_url(&oauth_state, &LOGIN_SCOPES)
        .await
        .map_err(|cause| {
            tracing::warn!(error = %cause, "could not reach the SSO");
            error(
                StatusCode::BAD_GATEWAY,
                "EVE's login is unavailable right now.",
            )
        })?;
    Ok((
        AppendHeaders([(
            SET_COOKIE,
            session::login_cookie(&oauth_state, return_to, sso.is_https()),
        )]),
        Redirect::to(&url),
    )
        .into_response())
}

/// `/eve/callback?code=…&state=…`: back from the SSO.
pub async fn callback(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let sso = sso_or_off(&state)?;
    let secure = sso.is_https();
    let clear_login = (SET_COOKIE, session::clear_cookie(LOGIN_COOKIE, secure));
    let failed = |why: &str| {
        tracing::info!(why, "a login failed");
        (
            AppendHeaders([clear_login.clone()]),
            Redirect::to(LOGIN_FAILED),
        )
            .into_response()
    };

    let cookie = session::cookie(&headers, LOGIN_COOKIE);
    let Some((expected, return_to)) = cookie.as_deref().and_then(session::parse_login_cookie)
    else {
        return Ok(failed("no login cookie"));
    };
    if query.get("state").map(String::as_str) != Some(expected) {
        return Ok(failed("the state does not match"));
    }
    // The player cancelled at the SSO.
    let Some(code) = query.get("code") else {
        return Ok(failed("no code"));
    };

    let tokens = match sso.exchange_code(code).await {
        Ok(tokens) => tokens,
        Err(cause) => {
            tracing::warn!(error = %cause, "the code exchange failed");
            return Ok(failed("code exchange"));
        }
    };
    let character = match sso.verify(&tokens.access_token).await {
        Ok(character) => character,
        Err(cause) => {
            tracing::warn!(error = %cause, "the SSO returned a token we do not accept");
            return Ok(failed("token check"));
        }
    };
    let token = session::log_in(&state.pool, &character, &tokens)
        .await
        .map_err(database_error)?;
    tracing::info!(character_id = character.id, "a character logged in");

    Ok((
        AppendHeaders([
            clear_login.clone(),
            (SET_COOKIE, session::session_cookie(&token, secure)),
        ]),
        Redirect::to(session::safe_return_path(Some(return_to))),
    )
        .into_response())
}

/// `POST /eve/logout`: ends the session; the character's last session
/// also takes their tokens and cached data, and revokes the refresh token.
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let dropped = session::log_out(&state.pool, &headers)
        .await
        .map_err(database_error)?;
    if let Some((character_id, refresh_token)) = dropped {
        cache::forget(&state.pool, &character::cache_prefix(character_id))
            .await
            .map_err(database_error)?;
        if let Some(sso) = &state.sso
            && let Err(cause) = sso.revoke(&refresh_token).await
        {
            tracing::warn!(error = %cause, "could not revoke a refresh token");
        }
    }
    let secure = state.sso.as_ref().is_some_and(SsoClient::is_https);
    Ok((
        AppendHeaders([(SET_COOKIE, session::clear_cookie(SESSION_COOKIE, secure))]),
        Redirect::to("/"),
    )
        .into_response())
}

#[derive(Serialize)]
pub struct Me {
    character: Option<CharacterOut>,
    /// Where a login starts (on the SSO callback's origin), `None` when this
    /// server has no SSO registration.
    login: Option<String>,
}

#[derive(Serialize)]
struct CharacterOut {
    id: i64,
    name: String,
}

/// `/api/me`: the logged-in character, if any.
pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Me>, Response> {
    let character = session::current(&state.pool, &headers)
        .await
        .map_err(database_error)?;
    Ok(Json(Me {
        character: character.map(|character| CharacterOut {
            id: character.id,
            name: character.name,
        }),
        login: state.sso.as_ref().map(SsoClient::login_url),
    }))
}
