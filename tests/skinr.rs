//! Browsing the Paragon Hub (`/api/skinr`, `/api/skinr/designers`) and a
//! ship's designs, through the real router over a seeded database. Needs
//! Postgres (`docker compose up -d postgres`).

mod common;

use axum::http::StatusCode;
use serde_json::Value;
use sqlx::PgPool;

const RIFTER: i32 = 587;
const SLASHER: i32 = 585;
const ALICE: i64 = 2_112_000_010;
const BOB: i64 = 2_112_000_011;

async fn seed(pool: &PgPool) {
    for statement in [
        "insert into ship_groups (id, name) values (25, 'Frigate')",
        "insert into ships (id, name, group_id, sof_hull, sof_faction, sof_race)
         values (587, 'Rifter', 25, 'mf4_t1', 'minmatarbase', 'minmatar'),
                (585, 'Slasher', 25, 'mf2_t1', 'minmatarbase', 'minmatar')",
        "insert into esi_characters (id, name) values (2112000010, 'Alice'), (2112000011, 'Bob')",
        // Alice: a Rifter design listed once at 100 PLEX, newest; another
        // whose only listing ran out. Bob: a Slasher design listed three times.
        "insert into skinr_designs (id, name, creator_id, ship_type_id, layout) values
            ('rust-bucket', 'Rust Bucket', 2112000010, 587, '{\"slots\": [{\"id\": 1, \"configuration\": {\"nanocoating\": {\"id\": 7}}}]}'),
            ('old-news', 'Old News', 2112000010, 587, '{\"slots\": []}'),
            ('night-shift', 'Night Shift', 2112000011, 585, '{\"slots\": []}')",
        "insert into skinr_listings (id, skinr_id, seller_id, state, price_plex, quantity,
            created, expires, last_modified) values
            ('l1', 'rust-bucket', 2112000010, 'listed', 100, 1, now() - interval '1 hour', now() + interval '1 day', now()),
            ('l2', 'old-news', 2112000010, 'listed', 10, 1, now() - interval '9 days', now() - interval '1 day', now()),
            ('l3', 'night-shift', 2112000011, 'listed', 50, 1, now() - interval '3 hours', now() + interval '1 day', now()),
            ('l4', 'night-shift', 2112000011, 'listed', 70, 2, now() - interval '4 hours', now() + interval '1 day', now()),
            ('l5', 'night-shift', 2112000011, 'listed', 90, 1, now() - interval '5 hours', now() + interval '1 day', now())",
        "insert into skinr_components (id, name, category, resource_file, color)
         values (7, 'Matte Rust', 'Material', 'res:/dx9/model/SpaceObjectFactory/materials/Rust_Matte.red', '#aa4422')",
        "insert into skinr_slot_materials (faction_id, slot_id, material_id) values (500002, 1, 4)",
    ] {
        sqlx::query(statement).execute(pool).await.unwrap();
    }
}

async fn browse(pool: &PgPool, query: &str) -> Value {
    let app = common::app(pool.clone(), "http://127.0.0.1:1", None);
    let response = common::get(&app, &format!("/api/skinr{query}"), None).await;
    assert_eq!(response.status(), StatusCode::OK, "{query}");
    common::json(response).await
}

fn ids(body: &Value) -> Vec<&str> {
    body["designs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|design| design["id"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn the_hub_lists_designs_on_sale_in_the_chosen_order() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;

    let newest = browse(&pool, "").await;
    assert_eq!(ids(&newest), vec!["rust-bucket", "night-shift"]);
    assert_eq!(newest["total"], 2);
    assert_eq!(newest["slot_materials"]["500002"]["1"], 4);
    assert_eq!(newest["components"]["7"]["material"], "rust_matte");
    assert_eq!(newest["designs"][1]["listings"], 3);
    assert_eq!(newest["designs"][1]["min_price_plex"], 50);
    assert_eq!(newest["designs"][0]["creator"]["name"], "Alice");

    let cheapest = browse(&pool, "?sort=cheapest").await;
    assert_eq!(ids(&cheapest), vec!["night-shift", "rust-bucket"]);
    let popular = browse(&pool, "?sort=popular").await;
    assert_eq!(ids(&popular), vec!["night-shift", "rust-bucket"]);

    // The busiest designers come along (one design each; Bob's sells more).
    let designers: Vec<i64> = newest["designers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|designer| designer["id"].as_i64().unwrap())
        .collect();
    assert_eq!(designers, vec![BOB, ALICE]);
}

#[tokio::test]
async fn the_hub_filters_by_designer_ship_and_text() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;

    let alice = browse(&pool, &format!("?designer={ALICE}")).await;
    assert_eq!(ids(&alice), vec!["rust-bucket"]);
    assert_eq!(alice["designer"]["name"], "Alice");
    assert_eq!(alice["designer"]["designs"], 1);
    assert_eq!(alice["designers"], Value::Array(vec![]));

    let slasher = browse(&pool, &format!("?ship={SLASHER}")).await;
    assert_eq!(ids(&slasher), vec!["night-shift"]);

    assert_eq!(ids(&browse(&pool, "?q=bob").await), vec!["night-shift"]);
    assert_eq!(ids(&browse(&pool, "?q=rifter").await), vec!["rust-bucket"]);
    assert_eq!(ids(&browse(&pool, "?q=100%25").await), Vec::<&str>::new());

    let unknown = browse(&pool, "?designer=1").await;
    assert_eq!(unknown["total"], 0);
    assert_eq!(unknown["designer"]["id"], 1);
}

#[tokio::test]
async fn designers_are_ranked_by_designs_then_listings() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    for statement in [
        "insert into skinr_designs (id, name, creator_id, ship_type_id, layout)
            values ('second', 'Second', 2112000010, 587, '{\"slots\": []}')",
        "insert into skinr_listings (id, skinr_id, seller_id, state, price_plex, quantity,
            created, expires, last_modified)
            values ('l6', 'second', 2112000010, 'listed', 5, 1, now(), now() + interval '1 day', now())",
    ] {
        sqlx::query(statement).execute(&pool).await.unwrap();
    }
    let app = common::app(pool, "http://127.0.0.1:1", None);

    let body = common::json(common::get(&app, "/api/skinr/designers", None).await).await;
    assert_eq!(body[0]["name"], "Alice");
    assert_eq!(body[0]["designs"], 2);
    assert_eq!(body[1]["name"], "Bob");
    assert_eq!(body[1]["listings"], 3);
}

#[tokio::test]
async fn a_ship_page_can_ask_for_an_unlisted_design() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let app = common::app(pool, "http://127.0.0.1:1", None);

    let listed =
        common::json(common::get(&app, &format!("/api/ships/{RIFTER}/skinrs"), None).await).await;
    assert_eq!(ids(&listed), vec!["rust-bucket"]);

    let with_owned = common::json(
        common::get(
            &app,
            &format!("/api/ships/{RIFTER}/skinrs?include=old-news"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(ids(&with_owned), vec!["rust-bucket", "old-news"]);
    assert_eq!(with_owned["designs"][1]["listings"], 0);

    // Only a design for this hull.
    let other = common::json(
        common::get(
            &app,
            &format!("/api/ships/{SLASHER}/skinrs?include=old-news"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(ids(&other), vec!["night-shift"]);
}

#[tokio::test]
async fn a_design_page_names_its_design_by_the_start_of_its_id() {
    let (pool, _db) = common::database().await;
    seed(&pool).await;
    let id = "0123456789abcdef".repeat(4);
    sqlx::query(
        "insert into skinr_designs (id, name, creator_id, ship_type_id, layout)
         values ($1, 'Hex Coat', 2112000010, 587, '{\"slots\": []}')",
    )
    .bind(&id)
    .execute(&pool)
    .await
    .unwrap();
    let app = common::app(pool, "http://127.0.0.1:1", None);
    let with = |include: &str| {
        let path = format!("/api/ships/{RIFTER}/skinrs?include={include}");
        let app = app.clone();
        async move {
            let body = common::json(common::get(&app, &path, None).await).await;
            ids(&body)
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<String>>()
        }
    };

    assert_eq!(with(&id[..12]).await, vec!["rust-bucket", id.as_str()]);
    assert_eq!(
        with(&id[..12].to_uppercase()).await,
        vec!["rust-bucket", id.as_str()]
    );
    // Too short to name one design, or not hex (no LIKE wildcards).
    assert_eq!(with(&id[..4]).await, vec!["rust-bucket"]);
    assert_eq!(with("01234567%").await, vec!["rust-bucket"]);
}
