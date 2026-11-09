//! Player-made SKINR designs, from the Paragon Hub sync (`crate::esi::skinr`)
//! joined with the SDE's component tables: a ship's designs, the whole hub
//! to browse, and its designers. `me` reuses the loaders for a character's
//! own designs.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::api::{database_error, not_found, png_response};
use crate::esi::skinr::LISTED;

/// Designs per page when browsing the hub.
const BROWSE_PAGE_SIZE: i64 = 48;
/// The most designers the designer list returns.
const MAX_DESIGNERS: i64 = 60;
/// Designers the browse page shows above its grid.
const FEATURED_DESIGNERS: i64 = 12;
/// The longest search text taken.
const MAX_QUERY_CHARS: usize = 64;

/// For each faction: SKINR material slot (1-4) to Trinity hull material (1-4).
pub type SlotMaterials = BTreeMap<i32, BTreeMap<i32, i32>>;

#[derive(Serialize)]
pub struct ShipSkinrs {
    /// For this hull's faction: SKINR material slot (1-4) to Trinity hull
    /// material (1-4). Empty when the SDE has no mapping for the faction.
    slot_materials: BTreeMap<i32, i32>,
    designs: Vec<DesignOut>,
    /// The components the designs use, by id.
    components: BTreeMap<i32, ComponentOut>,
}

#[derive(Serialize)]
pub struct DesignOut {
    pub id: String,
    name: String,
    line: Option<String>,
    tier: Option<i32>,
    pub ship_type_id: i32,
    pub creator: Person,
    /// The design's slots exactly as ESI returns them.
    layout: serde_json::Value,
    /// Live Paragon Hub listings, their total licenses, and the cheapest.
    listings: i64,
    quantity: i64,
    min_price_plex: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Person {
    pub id: i64,
    pub name: Option<String>,
}

#[derive(Serialize)]
pub struct ComponentOut {
    id: i32,
    name: String,
    category: String,
    finish: Option<String>,
    /// For materials: the SOF material name DNA uses (`material?…`).
    material: Option<String>,
    /// For patterns: the projection texture.
    texture: Option<String>,
    projection_u: Option<String>,
    projection_v: Option<String>,
    icon_url: Option<String>,
    /// For materials: a swatch color, `#rrggbb`.
    color: Option<String>,
}

#[derive(sqlx::FromRow)]
struct DesignRow {
    id: String,
    name: String,
    line: Option<String>,
    tier: Option<i32>,
    ship_type_id: i32,
    creator_id: i64,
    creator_name: Option<String>,
    layout: String,
    listings: i64,
    quantity: i64,
    min_price_plex: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct ComponentRow {
    id: i32,
    name: String,
    category: String,
    finish: Option<String>,
    resource_file: String,
    projection_u: Option<String>,
    projection_v: Option<String>,
    color: Option<String>,
    has_icon: bool,
}

/// The SOF name of a material file: its file name without extension.
fn sof_material_name(resource_file: &str) -> String {
    let file = resource_file.rsplit('/').next().unwrap_or(resource_file);
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    stem.to_lowercase()
}

/// Every component id a design's layout uses (nanocoatings and patterns).
fn component_ids(layout: &serde_json::Value, into: &mut BTreeSet<i32>) {
    let Some(slots) = layout.get("slots").and_then(|slots| slots.as_array()) else {
        return;
    };
    for slot in slots {
        let Some(configuration) = slot.get("configuration") else {
            continue;
        };
        for kind in ["nanocoating", "pattern"] {
            if let Some(id) = configuration
                .get(kind)
                .and_then(|part| part.get("id"))
                .and_then(|id| id.as_i64())
            {
                into.insert(id as i32);
            }
        }
    }
}

/// The stored designs of `ids`, in that order, with their live listings;
/// ids that are not stored are left out. The components they use are added
/// to `components`.
pub async fn load_designs(
    pool: &PgPool,
    ids: &[String],
    components: &mut BTreeSet<i32>,
) -> sqlx::Result<Vec<DesignOut>> {
    let rows = sqlx::query_as::<_, DesignRow>(
        "select d.id, d.name, d.line, d.tier, d.ship_type_id, d.creator_id,
                c.name as creator_name, d.layout,
                count(l.id) as listings, coalesce(sum(l.quantity), 0)::bigint as quantity,
                min(l.price_plex) as min_price_plex
         from skinr_designs d
         left join skinr_listings l on l.skinr_id = d.id and l.state = $2 and l.expires > now()
         left join esi_characters c on c.id = d.creator_id
         where d.id = any($1)
         group by d.id, c.name",
    )
    .bind(ids)
    .bind(LISTED)
    .fetch_all(pool)
    .await?;

    let mut by_id: HashMap<String, DesignOut> = HashMap::with_capacity(rows.len());
    for row in rows {
        let layout: serde_json::Value = serde_json::from_str(&row.layout).unwrap_or_default();
        component_ids(&layout, components);
        by_id.insert(
            row.id.clone(),
            DesignOut {
                id: row.id,
                name: row.name,
                line: row.line,
                tier: row.tier,
                ship_type_id: row.ship_type_id,
                creator: Person {
                    id: row.creator_id,
                    name: row.creator_name,
                },
                layout,
                listings: row.listings,
                quantity: row.quantity,
                min_price_plex: row.min_price_plex,
            },
        );
    }
    Ok(ids.iter().filter_map(|id| by_id.remove(id)).collect())
}

/// The components of `ids`, by id, as the frontend builds them.
pub async fn load_components(
    pool: &PgPool,
    ids: &BTreeSet<i32>,
) -> sqlx::Result<BTreeMap<i32, ComponentOut>> {
    let ids: Vec<i32> = ids.iter().copied().collect();
    Ok(sqlx::query_as::<_, ComponentRow>(
        "select c.id, c.name, c.category, c.finish, c.resource_file, c.projection_u, c.projection_v,
                c.color,
                i.component_id is not null as has_icon
         from skinr_components c
         left join skinr_component_icons i on i.component_id = c.id
         where c.id = any($1)",
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| {
        let is_pattern = row.category == "Pattern";
        (
            row.id,
            ComponentOut {
                id: row.id,
                material: (!is_pattern).then(|| sof_material_name(&row.resource_file)),
                texture: is_pattern.then(|| row.resource_file.to_lowercase()),
                icon_url: row.has_icon.then(|| format!("/api/skinr/icons/{}.png", row.id)),
                name: row.name,
                category: row.category,
                finish: row.finish,
                projection_u: row.projection_u,
                projection_v: row.projection_v,
                color: row.color,
            },
        )
    })
    .collect())
}

/// Every faction's slot mapping, for pages that show designs for many hulls.
pub async fn load_slot_materials(pool: &PgPool) -> sqlx::Result<SlotMaterials> {
    let mut all = SlotMaterials::new();
    for (faction, slot, material) in sqlx::query_as::<_, (i32, i32, i32)>(
        "select faction_id, slot_id, material_id from skinr_slot_materials",
    )
    .fetch_all(pool)
    .await?
    {
        all.entry(faction).or_default().insert(slot, material);
    }
    Ok(all)
}

#[derive(Deserialize)]
pub struct ShipSkinrsQuery {
    /// A design to show even when it is not listed (one the visitor owns):
    /// its id, or the start of it as a design page's URL carries it.
    include: Option<String>,
}

/// The shortest id prefix that names a design (a design page's URL carries
/// twelve characters; eight are already unique across every design).
const MIN_DESIGN_PREFIX: usize = 8;

/// The design for this hull that `wanted` names: its whole id, or a hex
/// prefix of at least [`MIN_DESIGN_PREFIX`] characters that only one of the
/// hull's designs starts with.
async fn design_for_ship(
    pool: &PgPool,
    ship_type_id: i32,
    wanted: &str,
) -> Result<Option<String>, sqlx::Error> {
    let exact: Option<String> =
        sqlx::query_scalar("select id from skinr_designs where id = $1 and ship_type_id = $2")
            .bind(wanted)
            .bind(ship_type_id)
            .fetch_optional(pool)
            .await?;
    if exact.is_some() {
        return Ok(exact);
    }
    // Hex only, so the prefix can never carry LIKE's wildcards.
    let is_prefix = wanted.len() >= MIN_DESIGN_PREFIX
        && wanted
            .chars()
            .all(|character| character.is_ascii_hexdigit());
    if !is_prefix {
        return Ok(None);
    }
    let matches: Vec<String> = sqlx::query_scalar(
        "select id from skinr_designs where ship_type_id = $2 and id like $1 || '%' limit 2",
    )
    .bind(wanted.to_ascii_lowercase())
    .bind(ship_type_id)
    .fetch_all(pool)
    .await?;
    Ok(match matches.as_slice() {
        [only] => Some(only.clone()),
        _ => None,
    })
}

/// `/api/ships/{id}/skinrs`: the designs listed for a ship and what they are made of.
pub async fn ship_skinrs(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
    Query(query): Query<ShipSkinrsQuery>,
) -> Result<Json<ShipSkinrs>, Response> {
    let faction_id: Option<Option<i32>> =
        sqlx::query_scalar("select faction_id from ships where id = $1")
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(database_error)?;
    let Some(faction_id) = faction_id else {
        return Err(not_found().await);
    };

    let slot_materials: BTreeMap<i32, i32> = sqlx::query_as::<_, (i32, i32)>(
        "select slot_id, material_id from skinr_slot_materials where faction_id = $1",
    )
    .bind(faction_id)
    .fetch_all(&pool)
    .await
    .map_err(database_error)?
    .into_iter()
    .collect();

    let mut ids: Vec<String> = sqlx::query_scalar(
        "select d.id from skinr_designs d
         join skinr_listings l on l.skinr_id = d.id and l.state = $2 and l.expires > now()
         where d.ship_type_id = $1
         group by d.id
         order by count(l.id) desc, d.name",
    )
    .bind(id)
    .bind(LISTED)
    .fetch_all(&pool)
    .await
    .map_err(database_error)?;
    if let Some(wanted) = query.include {
        let included = design_for_ship(&pool, id, &wanted)
            .await
            .map_err(database_error)?;
        if let Some(included) = included
            && !ids.contains(&included)
        {
            ids.push(included);
        }
    }

    let mut used = BTreeSet::new();
    let designs = load_designs(&pool, &ids, &mut used)
        .await
        .map_err(database_error)?;
    let components = load_components(&pool, &used)
        .await
        .map_err(database_error)?;

    Ok(Json(ShipSkinrs {
        slot_materials,
        designs,
        components,
    }))
}

/// How the hub's designs are ordered.
#[derive(Deserialize, Default, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Sort {
    /// The most recently listed first.
    #[default]
    Newest,
    /// The lowest asking price first.
    Cheapest,
    /// The most listings first.
    Popular,
}

impl Sort {
    fn order_by(self) -> &'static str {
        match self {
            Sort::Newest => "max(l.created) desc, d.id",
            Sort::Cheapest => "min(l.price_plex) asc nulls last, d.id",
            Sort::Popular => "count(l.id) desc, sum(l.quantity) desc, d.id",
        }
    }
}

#[derive(Deserialize)]
pub struct BrowseQuery {
    #[serde(default)]
    sort: Sort,
    designer: Option<i64>,
    ship: Option<i32>,
    /// Matches design, designer and hull names.
    q: Option<String>,
    /// From 1.
    page: Option<i64>,
}

#[derive(Serialize)]
pub struct Browse {
    designs: Vec<DesignOut>,
    components: BTreeMap<i32, ComponentOut>,
    slot_materials: SlotMaterials,
    /// Designs matching the filters, across all pages.
    total: i64,
    page: i64,
    page_size: i64,
    /// The designer filtered by, with their name.
    designer: Option<Designer>,
    /// The busiest designers, when not filtering by one.
    designers: Vec<Designer>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Designer {
    id: i64,
    name: Option<String>,
    /// Their designs listed now, and the listings of them.
    designs: i64,
    listings: i64,
}

/// A search text as an ILIKE pattern, its wildcards taken literally.
fn like_pattern(query: &str) -> Option<String> {
    let query: String = query.trim().chars().take(MAX_QUERY_CHARS).collect();
    if query.is_empty() {
        return None;
    }
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    Some(format!("%{escaped}%"))
}

async fn designers(pool: &PgPool, only: Option<i64>, limit: i64) -> sqlx::Result<Vec<Designer>> {
    sqlx::query_as(
        "select d.creator_id as id, c.name, count(distinct d.id) as designs, count(l.id) as listings
         from skinr_designs d
         join skinr_listings l on l.skinr_id = d.id and l.state = $1 and l.expires > now()
         left join esi_characters c on c.id = d.creator_id
         where ($2::bigint is null or d.creator_id = $2)
         group by d.creator_id, c.name
         order by count(distinct d.id) desc, count(l.id) desc, d.creator_id
         limit $3",
    )
    .bind(LISTED)
    .bind(only)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// `/api/skinr`: every design for sale on the Paragon Hub, filtered and paged.
pub async fn browse(
    State(pool): State<PgPool>,
    Query(query): Query<BrowseQuery>,
) -> Result<Json<Browse>, Response> {
    let page = query.page.unwrap_or(1).max(1);
    let pattern = query.q.as_deref().and_then(like_pattern);
    let rows: Vec<(String, i64)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "select d.id, count(*) over () as total
         from skinr_designs d
         join skinr_listings l on l.skinr_id = d.id and l.state = $1 and l.expires > now()
         left join esi_characters c on c.id = d.creator_id
         left join ships s on s.id = d.ship_type_id
         where ($2::bigint is null or d.creator_id = $2)
           and ($3::integer is null or d.ship_type_id = $3)
           and ($4::text is null or d.name ilike $4 or c.name ilike $4 or s.name ilike $4)
         group by d.id
         order by {}
         limit $5 offset $6",
        query.sort.order_by()
    )))
    .bind(LISTED)
    .bind(query.designer)
    .bind(query.ship)
    .bind(&pattern)
    .bind(BROWSE_PAGE_SIZE)
    .bind((page - 1) * BROWSE_PAGE_SIZE)
    .fetch_all(&pool)
    .await
    .map_err(database_error)?;
    let total = rows.first().map_or(0, |(_, total)| *total);
    let ids: Vec<String> = rows.into_iter().map(|(id, _)| id).collect();

    let mut used = BTreeSet::new();
    let designs = load_designs(&pool, &ids, &mut used)
        .await
        .map_err(database_error)?;
    let components = load_components(&pool, &used)
        .await
        .map_err(database_error)?;
    let slot_materials = load_slot_materials(&pool).await.map_err(database_error)?;

    let designer = match query.designer {
        Some(id) => Some(
            designers(&pool, Some(id), 1)
                .await
                .map_err(database_error)?
                .pop()
                .unwrap_or(Designer {
                    id,
                    name: sqlx::query_scalar("select name from esi_characters where id = $1")
                        .bind(id)
                        .fetch_optional(&pool)
                        .await
                        .map_err(database_error)?,
                    designs: 0,
                    listings: 0,
                }),
        ),
        None => None,
    };
    let designers = if designer.is_none() {
        designers(&pool, None, FEATURED_DESIGNERS)
            .await
            .map_err(database_error)?
    } else {
        Vec::new()
    };

    Ok(Json(Browse {
        designs,
        components,
        slot_materials,
        total,
        page,
        page_size: BROWSE_PAGE_SIZE,
        designer,
        designers,
    }))
}

/// `/api/skinr/designers`: the designers with designs on sale, busiest first.
pub async fn designer_list(State(pool): State<PgPool>) -> Result<Json<Vec<Designer>>, Response> {
    designers(&pool, None, MAX_DESIGNERS)
        .await
        .map(Json)
        .map_err(database_error)
}

/// `/api/skinr/icons/{component}.png`: the client's icon for a component.
pub async fn component_icon(
    State(pool): State<PgPool>,
    Path(file): Path<String>,
) -> Result<Response, Response> {
    let Some(id) = file
        .strip_suffix(".png")
        .and_then(|id| id.parse::<i32>().ok())
    else {
        return Err(not_found().await);
    };
    let png: Option<Vec<u8>> =
        sqlx::query_scalar("select png from skinr_component_icons where component_id = $1")
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(database_error)?;
    png_response(png, "Icon not found.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_files_become_sof_names() {
        assert_eq!(
            sof_material_name(
                "res:/dx9/model/SpaceObjectFactory/materials/cosm_BW_matt_000_000_100.red"
            ),
            "cosm_bw_matt_000_000_100"
        );
    }

    #[test]
    fn layouts_name_their_components() {
        let layout = serde_json::json!({"slots": [
            {"id": 1, "configuration": {"nanocoating": {"id": 1143}}},
            {"id": 5, "configuration": {"pattern": {"id": 188, "configuration": {}}}},
            {"id": 6, "configuration": {"nanocoating": {"id": 209}}}
        ]});
        let mut ids = BTreeSet::new();
        component_ids(&layout, &mut ids);
        assert_eq!(ids.into_iter().collect::<Vec<_>>(), vec![188, 209, 1143]);
    }

    #[test]
    fn search_text_is_matched_literally() {
        assert_eq!(like_pattern("  "), None);
        assert_eq!(like_pattern("Pac"), Some("%Pac%".to_owned()));
        assert_eq!(like_pattern("50%_off"), Some("%50\\%\\_off%".to_owned()));
    }
}
