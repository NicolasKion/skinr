//! `/sitemap.xml`: every page worth indexing, from the database, with the
//! URLs the frontend builds (`crate::urls`).

use std::fmt::Write as _;

use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use sqlx::PgPool;

use super::AppState;
use super::api::database_error;
use crate::esi::skinr::LISTED;
use crate::urls;

/// The site's origin when `PUBLIC_ORIGIN` is unset: the local stack.
const DEFAULT_PUBLIC_ORIGIN: &str = "https://skinr-space.test";
/// The most URLs one sitemap may hold (sitemaps.org).
const MAX_URLS: usize = 50_000;
/// The catalog changes with the SDE and the hub sync; an hour is fresh enough.
const CACHE_CONTROL: &str = "public, max-age=3600";

/// The origin from `PUBLIC_ORIGIN`, with the local default, without a trailing slash.
pub fn public_origin() -> String {
    std::env::var("PUBLIC_ORIGIN")
        .ok()
        .filter(|origin| !origin.is_empty())
        .unwrap_or_else(|| DEFAULT_PUBLIC_ORIGIN.to_owned())
        .trim_end_matches('/')
        .to_owned()
}

/// Every page's path: the home page and the hub, ships, their skins, designs
/// on sale and their designers.
async fn paths(pool: &PgPool) -> sqlx::Result<Vec<String>> {
    let mut paths = vec!["/".to_owned(), "/skinr".to_owned()];

    let ships: Vec<String> = sqlx::query_scalar("select name from ships order by name")
        .fetch_all(pool)
        .await?;
    paths.extend(ships.iter().map(|ship| urls::ship_path(ship)));

    let skins: Vec<(String, String)> = sqlx::query_as(
        "select s.name, k.name from ship_skins k join ships s on s.id = k.ship_id
         order by s.name, k.name",
    )
    .fetch_all(pool)
    .await?;
    paths.extend(skins.iter().map(|(ship, skin)| urls::skin_path(ship, skin)));

    let designs: Vec<(String, String, String)> = sqlx::query_as(
        "select s.name, d.name, d.id from skinr_designs d
         join ships s on s.id = d.ship_type_id
         where exists (select 1 from skinr_listings l
                       where l.skinr_id = d.id and l.state = $1 and l.expires > now())
         order by s.name, d.name, d.id",
    )
    .bind(LISTED)
    .fetch_all(pool)
    .await?;
    paths.extend(
        designs
            .iter()
            .map(|(ship, design, id)| urls::design_path(ship, design, id)),
    );

    let designers: Vec<(i64, Option<String>)> = sqlx::query_as(
        "select distinct d.creator_id, c.name from skinr_designs d
         left join esi_characters c on c.id = d.creator_id
         where exists (select 1 from skinr_listings l
                       where l.skinr_id = d.id and l.state = $1 and l.expires > now())
         order by d.creator_id",
    )
    .bind(LISTED)
    .fetch_all(pool)
    .await?;
    paths.extend(
        designers
            .iter()
            .map(|(id, name)| urls::designer_path(*id, name.as_deref())),
    );

    // Two designs can share a name and an id prefix only in theory; a
    // sitemap lists each URL once all the same.
    let mut seen = std::collections::HashSet::new();
    paths.retain(|path| seen.insert(path.clone()));
    Ok(paths)
}

/// The sitemap document for `paths` under `origin`.
fn document(origin: &str, paths: &[String]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
"#,
    );
    for path in paths.iter().take(MAX_URLS) {
        let _ = writeln!(
            xml,
            "<url><loc>{}</loc></url>",
            crate::og::escape(&format!("{origin}{path}"))
        );
    }
    xml.push_str("</urlset>\n");
    xml
}

/// `/sitemap.xml`.
pub async fn sitemap(State(state): State<AppState>) -> Response {
    let paths = match paths(&state.pool).await {
        Ok(paths) => paths,
        Err(cause) => return database_error(cause),
    };
    if paths.len() > MAX_URLS {
        tracing::warn!(
            urls = paths.len(),
            "sitemap: over the limit of one file, the rest is left out"
        );
    }
    (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/xml; charset=utf-8"),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static(CACHE_CONTROL),
            ),
        ],
        document(&state.public_origin, &paths),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_are_absolute_and_escaped() {
        let xml = document("https://skinr.space", &["/".into(), "/ships/a&b".into()]);
        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("<loc>https://skinr.space/</loc>"));
        assert!(xml.contains("<loc>https://skinr.space/ships/a&amp;b</loc>"));
    }
}
