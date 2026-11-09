//! Log in with EVE and the character's SKINR page, through the real router,
//! with EVE SSO and ESI replaced by one local mock on an ephemeral port.
//! Access tokens are RS256 JWTs signed with a test key the mock's JWKS
//! publishes. Needs Postgres (`docker compose up -d postgres`).

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use serde_json::{Value, json};
use skinr::auth::sso::SsoClient;

const CLIENT_ID: &str = "test-client";
const CLIENT_SECRET: &str = "test-secret";
const CALLBACK: &str = "http://localhost:5200/eve/callback";
const CHARACTER_ID: i64 = 2_112_000_001;
const CHARACTER_NAME: &str = "Test Pilot";
const CORPORATION_ID: i64 = 98_000_001;
const ALLIANCE_ID: i64 = 99_000_001;
const SELLER_ID: i64 = 2_112_000_002;
const KEY_ID: &str = "JWT-Signature-Key";
const PRIVATE_KEY: &[u8] = include_bytes!("fixtures/sso-test-key.pem");
const JWKS: &str = include_str!("fixtures/sso-test-jwks.json");

/// What the mock hands out and what it saw.
#[derive(Default)]
struct Mock {
    base: String,
    /// Seconds the next access token is valid for.
    expires_in: i64,
    /// Refuse refresh grants like a revoked refresh token.
    refuse_refresh: bool,
    issued: u32,
    refreshes: u32,
    revoked: Vec<String>,
    /// The access tokens ESI's authed routes were called with.
    bearers: Vec<String>,
}

type Shared = Arc<Mutex<Mock>>;

fn sign(audience: &[&str]) -> String {
    let expires = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 1200;
    let claims = json!({
        "sub": format!("CHARACTER:EVE:{CHARACTER_ID}"),
        "name": CHARACTER_NAME,
        "owner": "owner-hash",
        "scp": "esi.cosmetic.char:read",
        "aud": audience,
        "iss": "https://login.eveonline.com",
        "exp": expires,
    });
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some(KEY_ID.to_owned());
    jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_rsa_pem(PRIVATE_KEY).unwrap(),
    )
    .unwrap()
}

fn listing(id: &str, skinr_id: &str, seller: i64) -> Value {
    json!({
        "id": id, "skinr_id": skinr_id, "seller_id": seller, "state": "listed",
        "price": {"plex": 120}, "quantity": 2,
        "created": "2026-10-10T09:40:02Z", "expires": "2099-11-09T09:40:02Z",
        "last_modified": "2026-10-10T09:40:02Z",
        "target": {"corporation_id": CORPORATION_ID}
    })
}

/// One page of listings for `after=0`, an empty one after it.
fn pages(query: &HashMap<String, String>, listings: Vec<Value>) -> Json<Value> {
    Json(match query.get("after").map(String::as_str) {
        Some("0") => json!({"cursor": {"after": "next"}, "listings": listings}),
        _ => json!({"cursor": {"after": "next"}, "listings": []}),
    })
}

/// Fails unless the request carries the current access token.
fn authed(mock: &Shared, headers: &HeaderMap) -> Result<(), StatusCode> {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_owned();
    let mut mock = mock.lock().unwrap();
    mock.bearers.push(bearer);
    Ok(())
}

fn basic_auth_ok(headers: &HeaderMap) -> bool {
    // base64("test-client:test-secret")
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        == Some("Basic dGVzdC1jbGllbnQ6dGVzdC1zZWNyZXQ=")
}

async fn token(
    State(mock): State<Shared>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    if !basic_auth_ok(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let mut mock = mock.lock().unwrap();
    match form.get("grant_type").map(String::as_str) {
        Some("authorization_code") if form.get("code").map(String::as_str) == Some("good-code") => {
        }
        Some("refresh_token") if mock.refuse_refresh => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid_grant"})),
            )
                .into_response();
        }
        Some("refresh_token") => mock.refreshes += 1,
        _ => return StatusCode::BAD_REQUEST.into_response(),
    }
    mock.issued += 1;
    Json(json!({
        "access_token": sign(&[CLIENT_ID, "EVE Online"]),
        "refresh_token": format!("refresh-{}", mock.issued),
        "token_type": "Bearer",
        "expires_in": mock.expires_in,
    }))
    .into_response()
}

async fn mock_server(mock: Shared) -> String {
    let app = Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(|State(mock): State<Shared>| async move {
                let base = mock.lock().unwrap().base.clone();
                Json(json!({
                    "issuer": "https://login.eveonline.com",
                    "authorization_endpoint": format!("{base}/v2/oauth/authorize"),
                    "token_endpoint": format!("{base}/v2/oauth/token"),
                    "jwks_uri": format!("{base}/oauth/jwks"),
                    "revocation_endpoint": format!("{base}/v2/oauth/revoke"),
                }))
            }),
        )
        .route(
            "/oauth/jwks",
            get(|| async { ([(header::CONTENT_TYPE, "application/json")], JWKS) }),
        )
        .route("/v2/oauth/token", post(token))
        .route(
            "/v2/oauth/revoke",
            post(
                |State(mock): State<Shared>, Form(form): Form<HashMap<String, String>>| async move {
                    mock.lock().unwrap().revoked.push(form["token"].clone());
                    StatusCode::OK
                },
            ),
        )
        .route(
            "/characters/{id}",
            get(|Path(id): Path<i64>| async move {
                Json(json!({
                    "name": CHARACTER_NAME, "corporation_id": CORPORATION_ID,
                    "alliance_id": ALLIANCE_ID, "character_id": id
                }))
            }),
        )
        .route(
            "/characters/{id}/cosmetics/skinr",
            get(|State(mock): State<Shared>, headers: HeaderMap| async move {
                authed(&mock, &headers)?;
                Ok::<_, StatusCode>((
                    [(header::CACHE_CONTROL, "public, max-age=3600"), (header::ETAG, "\"l1\"")],
                    Json(json!({"licenses": [
                        {"skinr_id": "owned-design", "activated": true, "unactivated": 3}
                    ]})),
                ))
            }),
        )
        .route(
            "/characters/{id}/cosmetics/skinr/components",
            get(|State(mock): State<Shared>, headers: HeaderMap| async move {
                authed(&mock, &headers)?;
                Ok::<_, StatusCode>(Json(json!({"licenses": [
                    {"component_id": 1143, "type": "nanocoating", "runs": {"remaining": 5}}
                ]})))
            }),
        )
        .route(
            "/characters/{id}/paragon-hub/skinr",
            get(
                |State(mock): State<Shared>,
                 headers: HeaderMap,
                 Query(query): Query<HashMap<String, String>>| async move {
                    authed(&mock, &headers)?;
                    Ok::<_, StatusCode>(pages(
                        &query,
                        vec![listing("my-listing", "made-design", CHARACTER_ID)],
                    ))
                },
            ),
        )
        .route(
            "/paragon-hub/skinr/characters/{id}",
            get(
                |State(mock): State<Shared>,
                 headers: HeaderMap,
                 Query(query): Query<HashMap<String, String>>| async move {
                    authed(&mock, &headers)?;
                    Ok::<_, StatusCode>(pages(&query, vec![]))
                },
            ),
        )
        .route(
            "/paragon-hub/skinr/corporations/{id}",
            get(
                |State(mock): State<Shared>,
                 headers: HeaderMap,
                 Path(id): Path<i64>,
                 Query(query): Query<HashMap<String, String>>| async move {
                    authed(&mock, &headers)?;
                    assert_eq!(id, CORPORATION_ID);
                    Ok::<_, StatusCode>(pages(
                        &query,
                        vec![listing("corp-offer", "offered-design", SELLER_ID)],
                    ))
                },
            ),
        )
        .route(
            "/paragon-hub/skinr/alliances/{id}",
            get(
                |State(mock): State<Shared>,
                 headers: HeaderMap,
                 Query(query): Query<HashMap<String, String>>| async move {
                    authed(&mock, &headers)?;
                    Ok::<_, StatusCode>(pages(&query, vec![]))
                },
            ),
        )
        .route(
            "/cosmetics/skinr/{id}",
            get(|Path(id): Path<String>| async move {
                let creator = if id == "made-design" { CHARACTER_ID } else { SELLER_ID };
                Json(json!({
                    "id": id, "name": format!("Design {id}"), "creator_id": creator,
                    "ship_type_id": 587, "tier": {"level": 2},
                    "layout": {"slots": [{"id": 1, "configuration": {"nanocoating": {"id": 1143}}}]}
                }))
            }),
        )
        .route(
            "/universe/names",
            post(|Json(ids): Json<Vec<i64>>| async move {
                Json(
                    ids.iter()
                        .map(|id| json!({"id": id, "name": format!("pilot {id}"), "category": "character"}))
                        .collect::<Vec<_>>(),
                )
            }),
        )
        .with_state(mock.clone());
    let base = common::serve(app).await;
    mock.lock().unwrap().base = base.clone();
    base
}

struct Setup {
    app: Router,
    mock: Shared,
    pool: sqlx::PgPool,
    _db: common::TestDb,
}

async fn setup(expires_in: i64) -> Setup {
    let (pool, db) = common::database().await;
    let mock: Shared = Arc::new(Mutex::new(Mock {
        expires_in,
        ..Mock::default()
    }));
    let base = mock_server(mock.clone()).await;
    let sso = SsoClient::new(&base, CLIENT_ID, CLIENT_SECRET, CALLBACK);
    Setup {
        app: common::app(pool.clone(), &base, Some(sso)),
        mock,
        pool,
        _db: db,
    }
}

fn set_cookie(response: &Response, name: &str) -> Option<String> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|cookie| cookie.starts_with(&format!("{name}=")))
        .map(str::to_owned)
}

fn cookie_value(set_cookie: &str) -> String {
    set_cookie.split(';').next().unwrap().to_owned()
}

fn location(response: &Response) -> String {
    response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned()
}

/// Logs in through the SSO round trip; returns the session cookie pair.
async fn log_in(setup: &Setup) -> String {
    let login = common::get(&setup.app, "/eve/login?return=/me", None).await;
    assert_eq!(login.status(), StatusCode::SEE_OTHER);
    let authorize = reqwest::Url::parse(&location(&login)).unwrap();
    let params: HashMap<_, _> = authorize.query_pairs().into_owned().collect();
    assert_eq!(params["client_id"], CLIENT_ID);
    assert_eq!(params["redirect_uri"], CALLBACK);
    assert_eq!(params["scope"], "esi.cosmetic.char:read");
    assert_eq!(params["response_type"], "code");
    let login_cookie = set_cookie(&login, "skinr_login").expect("login cookie");
    assert!(login_cookie.contains("HttpOnly"));

    let callback = common::get(
        &setup.app,
        &format!("/eve/callback?code=good-code&state={}", params["state"]),
        Some(&cookie_value(&login_cookie)),
    )
    .await;
    assert_eq!(callback.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&callback), "/me");
    let session = set_cookie(&callback, "skinr_session").expect("session cookie");
    assert!(session.contains("HttpOnly") && session.contains("SameSite=Lax"));
    // http://localhost: no Secure flag, or the browser would drop it.
    assert!(!session.contains("Secure"));
    cookie_value(&session)
}

#[tokio::test]
async fn logging_in_opens_a_session_and_keeps_tokens_out_of_the_browser() {
    let setup = setup(1199).await;
    let session = log_in(&setup).await;

    let me = common::json(common::get(&setup.app, "/api/me", Some(&session)).await).await;
    assert_eq!(me["character"]["id"], CHARACTER_ID);
    assert_eq!(me["character"]["name"], CHARACTER_NAME);
    assert_eq!(me["login"], "http://localhost:5200/eve/login");
    assert!(
        !me.to_string().contains("refresh-"),
        "no token in the answer"
    );

    // The session is stored hashed, the tokens server-side.
    let stored: String = sqlx::query_scalar("select token_hash from sessions")
        .fetch_one(&setup.pool)
        .await
        .unwrap();
    assert!(!session.ends_with(&stored));
    let refresh: String = sqlx::query_scalar("select refresh_token from sso_characters")
        .fetch_one(&setup.pool)
        .await
        .unwrap();
    assert_eq!(refresh, "refresh-1");
}

#[tokio::test]
async fn a_callback_with_the_wrong_state_is_refused() {
    let setup = setup(1199).await;
    let login = common::get(&setup.app, "/eve/login", None).await;
    let login_cookie = cookie_value(&set_cookie(&login, "skinr_login").unwrap());

    let callback = common::get(
        &setup.app,
        "/eve/callback?code=good-code&state=forged",
        Some(&login_cookie),
    )
    .await;
    assert_eq!(location(&callback), "/me?login=failed");
    assert!(set_cookie(&callback, "skinr_session").is_none());
    assert_eq!(
        setup.mock.lock().unwrap().issued,
        0,
        "the code was never spent"
    );
}

#[tokio::test]
async fn a_token_for_another_app_is_refused() {
    let setup = setup(1199).await;
    let sso = SsoClient::new(
        &setup.mock.lock().unwrap().base.clone(),
        CLIENT_ID,
        CLIENT_SECRET,
        CALLBACK,
    );
    assert!(
        sso.verify(&sign(&["other-app", "EVE Online"]))
            .await
            .is_err()
    );
    assert!(sso.verify(&sign(&[CLIENT_ID])).await.is_err());
    let character = sso.verify(&sign(&[CLIENT_ID, "EVE Online"])).await.unwrap();
    assert_eq!(character.id, CHARACTER_ID);
    assert_eq!(character.scopes, vec!["esi.cosmetic.char:read"]);
}

#[tokio::test]
async fn the_skinr_page_gathers_licenses_listings_and_offers() {
    let setup = setup(1199).await;
    let session = log_in(&setup).await;

    let response = common::get(&setup.app, "/api/me/skinr", Some(&session)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = common::json(response).await;

    assert_eq!(body["character"]["corporation_id"], CORPORATION_ID);
    assert_eq!(body["character"]["alliance_id"], ALLIANCE_ID);
    assert_eq!(body["licenses"][0]["skinr_id"], "owned-design");
    assert_eq!(body["licenses"][0]["unactivated"], 3);
    assert_eq!(body["component_licenses"][0]["component_id"], 1143);
    assert_eq!(body["component_licenses"][0]["runs"], 5);
    assert_eq!(body["listings"][0]["id"], "my-listing");
    assert_eq!(body["listings"][0]["price_plex"], 120);
    assert_eq!(body["offers"][0]["audience"], "corporation");
    assert_eq!(
        body["offers"][0]["seller"]["name"],
        format!("pilot {SELLER_ID}")
    );
    assert_eq!(body["created"], json!(["made-design"]));
    assert_eq!(body["unavailable"], json!([]));

    // Every design mentioned was fetched once and comes with its creator.
    let designs: Vec<&str> = body["designs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|design| design["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        designs,
        vec!["owned-design", "made-design", "offered-design"]
    );
    assert_eq!(
        body["designs"][1]["creator"]["name"],
        format!("pilot {CHARACTER_ID}")
    );
    assert_eq!(body["designs"][0]["ship_type_id"], 587);

    // A second visit is answered from the cache.
    let calls = setup.mock.lock().unwrap().bearers.len();
    let again = common::get(&setup.app, "/api/me/skinr", Some(&session)).await;
    assert_eq!(again.status(), StatusCode::OK);
    assert_eq!(setup.mock.lock().unwrap().bearers.len(), calls);
}

#[tokio::test]
async fn an_expiring_access_token_is_refreshed_first() {
    let setup = setup(0).await;
    let session = log_in(&setup).await;
    setup.mock.lock().unwrap().expires_in = 1199;

    let response = common::get(&setup.app, "/api/me/skinr", Some(&session)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(setup.mock.lock().unwrap().refreshes, 1);
    let refresh: String = sqlx::query_scalar("select refresh_token from sso_characters")
        .fetch_one(&setup.pool)
        .await
        .unwrap();
    assert_eq!(refresh, "refresh-2", "the rotated refresh token is kept");
}

#[tokio::test]
async fn a_refused_refresh_logs_the_character_out() {
    let setup = setup(0).await;
    let session = log_in(&setup).await;
    setup.mock.lock().unwrap().refuse_refresh = true;

    let response = common::get(&setup.app, "/api/me/skinr", Some(&session)).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let me = common::json(common::get(&setup.app, "/api/me", Some(&session)).await).await;
    assert_eq!(me["character"], Value::Null);
}

#[tokio::test]
async fn logging_out_ends_the_session_and_revokes_the_token() {
    let setup = setup(1199).await;
    let session = log_in(&setup).await;
    common::get(&setup.app, "/api/me/skinr", Some(&session)).await;

    let response = common::send(
        &setup.app,
        Request::post("/eve/logout")
            .header(header::COOKIE, &session)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(
        set_cookie(&response, "skinr_session")
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(setup.mock.lock().unwrap().revoked, vec!["refresh-1"]);

    let me = common::json(common::get(&setup.app, "/api/me", Some(&session)).await).await;
    assert_eq!(me["character"], Value::Null);
    let left: i64 = sqlx::query_scalar(
        "select (select count(*) from sso_characters) + (select count(*) from esi_cache
            where key like 'character:%')",
    )
    .fetch_one(&setup.pool)
    .await
    .unwrap();
    assert_eq!(left, 0, "tokens and cached data are gone");
}

#[tokio::test]
async fn without_a_registration_login_is_off() {
    let (pool, _db) = common::database().await;
    let app = common::app(pool, "http://127.0.0.1:1", None);

    let login = common::get(&app, "/eve/login", None).await;
    assert_eq!(login.status(), StatusCode::SERVICE_UNAVAILABLE);
    let me = common::json(common::get(&app, "/api/me", None).await).await;
    assert_eq!(me, json!({"character": null, "login": null}));
    let mine = common::get(&app, "/api/me/skinr", None).await;
    assert_eq!(mine.status(), StatusCode::UNAUTHORIZED);
}
