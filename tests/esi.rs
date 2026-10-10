//! The ESI client and the SKINR fetches, against a local mock of ESI on an
//! ephemeral port.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use skinr::esi::client::{COMPATIBILITY_DATE, EsiClient};
use skinr::esi::skinr::{fetch_design, fetch_listings, fetch_names};

const USER_AGENT: &str = "skinr.space-test/1.0";

async fn serve(app: Router) -> EsiClient {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    EsiClient::new(format!("http://{address}"), USER_AGENT)
}

/// Fails the request unless it carries the compatibility date and user agent.
fn identified(headers: &HeaderMap) -> Result<(), StatusCode> {
    let date = headers
        .get("X-Compatibility-Date")
        .and_then(|v| v.to_str().ok());
    let agent = headers.get("User-Agent").and_then(|v| v.to_str().ok());
    if date == Some(COMPATIBILITY_DATE) && agent == Some(USER_AGENT) {
        Ok(())
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

#[tokio::test]
async fn listings_are_paged_by_cursor_and_requests_identify_themselves() {
    let esi = serve(Router::new().route(
        "/paragon-hub/skinr",
        get(|headers: HeaderMap, Query(query): Query<std::collections::HashMap<String, String>>| async move {
            identified(&headers)?;
            if query.get("limit").map(String::as_str) != Some("100") {
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
            let page = match query.get("after").map(String::as_str) {
                Some("0") => json!({
                    "cursor": {"after": "c1", "before": "b1"},
                    "listings": [{
                        "id": "l1", "skinr_id": "s1", "seller_id": 7, "state": "listed",
                        "price": {"plex": 60}, "quantity": 1,
                        "created": "2026-10-10T09:40:02Z", "expires": "2026-11-09T09:40:02Z",
                        "last_modified": "2026-10-10T09:40:02Z"
                    }]
                }),
                _ => json!({"cursor": {"after": "c1"}, "listings": []}),
            };
            Ok(Json(page))
        }),
    ))
    .await;

    let first = fetch_listings(&esi, "0").await.unwrap();
    assert_eq!(first.listings.len(), 1);
    assert_eq!(first.listings[0].price_plex(), Some(60));
    let next = first.cursor.and_then(|cursor| cursor.after).unwrap();
    assert_eq!(next, "c1");
    assert!(
        fetch_listings(&esi, &next)
            .await
            .unwrap()
            .listings
            .is_empty()
    );
}

#[tokio::test]
async fn a_rate_limited_request_waits_and_retries() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let esi = serve(Router::new().route(
        "/cosmetics/skinr/{id}",
        get(move |Path(id): Path<String>| {
            let counter = counter.clone();
            async move {
                if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                    return Err((StatusCode::TOO_MANY_REQUESTS, [("Retry-After", "0")]));
                }
                Ok(Json(json!({
                    "id": id, "name": "Pacoman", "line": "Destroyer of Pills",
                    "creator_id": 9, "ship_type_id": 95741, "tier": {"level": 5},
                    "layout": {"slots": [], "pattern_blend_mode": "normal"}
                })))
            }
        }),
    ))
    .await;

    let design = fetch_design(&esi, "s1").await.unwrap();
    assert_eq!(design.name, "Pacoman");
    assert_eq!(design.tier.map(|tier| tier.level), Some(5));
    assert_eq!(calls.load(Ordering::SeqCst), 2, "one 429, then the retry");
}

#[tokio::test]
async fn names_drop_only_the_unknown_id() {
    let esi = serve(Router::new().route(
        "/universe/names",
        post(|Json(ids): Json<Vec<i64>>| async move {
            // ESI rejects a whole batch that holds an ID it does not know.
            if ids.contains(&404) {
                return Err(StatusCode::NOT_FOUND);
            }
            Ok(Json(
                ids.iter()
                    .map(|id| json!({"id": id, "name": format!("pilot {id}"), "category": "character"}))
                    .collect::<Vec<_>>(),
            ))
        }),
    ))
    .await;

    let mut names = fetch_names(&esi, &[1, 2, 404, 3]).await;
    names.sort_by_key(|name| name.id);
    let ids: Vec<i64> = names.iter().map(|name| name.id).collect();
    assert_eq!(ids, vec![1, 2, 3]);
    assert_eq!(names[0].name, "pilot 1");
}
