//! The card of each page type: what it shows, read from the database, and
//! what it depends on, which names its file.

use serde_json::json;
use sqlx::PgPool;

use super::cards::{Art, Card};
use super::hexagon::{Badge, Bands, DEFAULT_LIGHTS, Hexagon, Pattern};
use super::renders::Job;
use super::svg::Color;
use super::{Freshness, Og, Rendered};
use crate::dna;
use crate::esi::skinr::LISTED;
use crate::sde::skin_icon_colors::IconColors;
use crate::thumbnails::Subject;

/// The accent each race's cards are tinted with, by SDE race id; the same
/// as the site's (`frontend/src/lib/eve.ts`, `RACE_TINTS`).
const RACE_TINTS: [(i32, &str); 9] = [
    (1, "#7fb2e5"),   // Caldari
    (2, "#e0875a"),   // Minmatar
    (4, "#e8c170"),   // Amarr
    (8, "#7fd1a8"),   // Gallente
    (16, "#b7a6ff"),  // Jove
    (32, "#ef6b6b"),  // Pirate
    (128, "#f2c14e"), // ORE
    (135, "#e2564b"), // Triglavian
    (168, "#9fb3c8"), // Upwell
];
/// A neutral silver for ships without a race, and the site's own cards.
const DEFAULT_TINT: &str = "#a1b5c9";
/// The ships on the home card, front first: Megathron, Raven, Apocalypse.
const HOME_SHIPS: [i32; 3] = [641, 638, 642];
/// How many designs the hub and designer cards show.
const GALLERY_DESIGNS: i64 = 3;
/// SKINR's pattern slots, each with the material slot it is painted in
/// (`frontend/src/lib/skinr.ts`, `PATTERN_SLOTS`).
const PATTERN_SLOTS: [(i64, i64); 2] = [(5, 6), (7, 8)];
/// The material slots a design's badge reads: its bands and its patterns' paint.
const BADGE_MATERIAL_SLOTS: [i64; 6] = [1, 2, 3, 4, 6, 8];
/// The paint of a pattern whose material slot is empty (`skinr.ts`).
const BARE_PATTERN: &str = "#d8d8d8";

fn tint(race_id: Option<i32>) -> Color {
    let hex = race_id
        .and_then(|race| RACE_TINTS.iter().find(|(id, _)| *id == race))
        .map_or(DEFAULT_TINT, |(_, hex)| hex);
    Color::constant(hex)
}

/// A colour from the database, if it is `#rrggbb`.
fn color(hex: Option<&str>) -> Option<Color> {
    hex.and_then(Color::parse)
}

/// A count with thousands separators: 12,345.
fn thousands(value: i64) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if value < 0 {
        out.push('-');
    }
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// "1 design", "3 designs".
fn plural(count: i64, one: &str, many: &str) -> String {
    format!(
        "{} {}",
        thousands(count),
        if count == 1 { one } else { many }
    )
}

/// The component a design's layout puts in `slot`: a `nanocoating` (a
/// material) or a `pattern`.
fn slot_component(layout: &serde_json::Value, slot: i64, kind: &str) -> Option<i32> {
    layout
        .get("slots")?
        .as_array()?
        .iter()
        .find(|candidate| candidate.get("id").and_then(serde_json::Value::as_i64) == Some(slot))?
        .pointer(&format!("/configuration/{kind}/id"))?
        .as_i64()
        .and_then(|id| i32::try_from(id).ok())
}

/// A design's SKIN badge, as the site draws it (`skinrIcon` in
/// `frontend/src/lib/skinr.ts`): its primary, secondary and detailing
/// materials tint the bottom, top and middle bands, and each pattern is laid
/// over it in its own material's colour. Also returns what it depends on.
async fn design_badge(
    pool: &PgPool,
    layout: &serde_json::Value,
) -> sqlx::Result<(Badge, serde_json::Value)> {
    let materials: Vec<i32> = BADGE_MATERIAL_SLOTS
        .iter()
        .filter_map(|slot| slot_component(layout, *slot, "nanocoating"))
        .collect();
    let found: Vec<(i32, Option<String>)> =
        sqlx::query_as("select id, color from skinr_components where id = any($1)")
            .bind(&materials)
            .fetch_all(pool)
            .await?;
    let color_of = |slot: i64| {
        let id = slot_component(layout, slot, "nanocoating")?;
        let (_, hex) = found.iter().find(|(found, _)| *found == id)?;
        color(hex.as_deref())
    };
    let bands = Bands {
        primary: color_of(1),
        secondary: color_of(2).or_else(|| color_of(1)),
        hull: color_of(3).or_else(|| color_of(4)).or_else(|| color_of(1)),
    };
    let mut patterns = Vec::new();
    let mut key = Vec::new();
    for (slot, material) in PATTERN_SLOTS {
        let Some(id) = slot_component(layout, slot, "pattern") else {
            continue;
        };
        let mask: Option<Vec<u8>> =
            sqlx::query_scalar("select png from skinr_component_icons where component_id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?;
        let Some(mask) = mask else { continue };
        let paint = color_of(material).unwrap_or_else(|| Color::constant(BARE_PATTERN));
        key.push(json!([id, paint]));
        patterns.push(Pattern { mask, color: paint });
    }
    let depends_on = json!([bands, key]);
    let badge = Badge::Hexagon(Hexagon {
        bands,
        lights: Color::constant(DEFAULT_LIGHTS),
        patterns,
    });
    Ok((badge, depends_on))
}

/// An in-game skin's SKIN badge, as the site picks it (`skin-icon.svelte`):
/// drawn from the colours the import read off its icon, else the client's
/// icon itself, else drawn from the skin's material colours.
fn skin_badge(icon_colors: Option<&str>, icon: Option<Vec<u8>>, colors: Bands) -> Badge {
    let read: Option<IconColors> = icon_colors.and_then(|text| serde_json::from_str(text).ok());
    match (read, icon) {
        (Some(read), _) => Badge::Hexagon(Hexagon {
            bands: Bands {
                primary: Color::parse(&read.primary),
                secondary: Color::parse(&read.secondary),
                hull: Color::parse(&read.hull),
            },
            lights: Color::parse(&read.lights).unwrap_or_else(|| Color::constant(DEFAULT_LIGHTS)),
            patterns: Vec::new(),
        }),
        (None, Some(png)) => Badge::Icon(png),
        (None, None) => Badge::Hexagon(Hexagon {
            bands: colors,
            lights: Color::constant(DEFAULT_LIGHTS),
            patterns: Vec::new(),
        }),
    }
}

/// A design ready to be drawn: its render job and what to show without one.
#[derive(sqlx::FromRow)]
struct DesignArt {
    id: String,
    ship_type_id: i32,
    layout: String,
    sof_hull: String,
    sof_faction: String,
    sof_race: String,
}

impl DesignArt {
    /// The design's own thumbnail (`crate::thumbnails`) and the look it must show.
    fn own(&self) -> Own {
        Own {
            subject: Subject::Design {
                id: self.id.clone(),
                ship: self.ship_type_id,
            },
            key: dna::design_look(
                &self.sof_hull,
                &self.sof_faction,
                &self.sof_race,
                &self.layout,
            ),
        }
    }

    fn job(&self) -> Job {
        Job::Skinr {
            design_id: self.id.clone(),
            ship_id: self.ship_type_id,
            look: dna::design_look(
                &self.sof_hull,
                &self.sof_faction,
                &self.sof_race,
                &self.layout,
            ),
        }
    }
}

/// A livery's own thumbnail, and what it must have been drawn from to count.
struct Own {
    subject: Subject,
    key: String,
}

const DESIGN_ART_SELECT: &str =
    "select d.id, d.ship_type_id, d.layout, s.sof_hull, s.sof_faction, s.sof_race
     from skinr_designs d join ships s on s.id = d.ship_type_id";

impl Og {
    /// A livery's art without waiting: its stored render, or its own current
    /// thumbnail.
    async fn ready_art(&self, job: &Job, own: Option<&Own>) -> Option<Vec<u8>> {
        if let Some(png) = self.renders().stored(job).await {
            return Some(png.to_vec());
        }
        let own = own?;
        crate::thumbnails::current_png(&self.config().thumbnails, &own.subject, &own.key).await
    }

    /// A livery's art: [`Self::ready_art`], or its render if that is drawn
    /// within the wait.
    async fn art(&self, job: &Job, own: Option<&Own>) -> Option<Vec<u8>> {
        if let Some(png) = self.ready_art(job, own).await {
            return Some(png);
        }
        let wait = self.config().render_wait;
        self.renders().get(job, wait).await.map(|png| png.to_vec())
    }

    /// Each design's art, or the thumbnail of its hull while there is none;
    /// the flag says whether every design had its own.
    async fn gallery(&self, designs: &[DesignArt]) -> (Vec<Vec<u8>>, bool) {
        let renders = futures_util::future::join_all(
            designs
                .iter()
                .map(|design| async move { self.art(&design.job(), Some(&design.own())).await }),
        )
        .await;
        let mut all = true;
        let mut pictures = Vec::with_capacity(designs.len());
        for (design, render) in designs.iter().zip(renders) {
            match render {
                Some(png) => pictures.push(png),
                None => {
                    all = false;
                    if let Some((png, _)) = self.thumbnail(design.ship_type_id).await {
                        pictures.push(png);
                    }
                }
            }
        }
        (pictures, all)
    }

    /// The site's card.
    pub async fn home(&self, pool: &PgPool) -> sqlx::Result<Rendered> {
        let (ships, skins, designs): (i64, i64, i64) = sqlx::query_as(
            "select (select count(*) from ships), (select count(*) from ship_skins),
                    (select count(distinct skinr_id) from skinr_listings
                     where state = $1 and expires > now())",
        )
        .bind(LISTED)
        .fetch_one(pool)
        .await?;
        let mut pictures = Vec::new();
        let mut stamps = Vec::new();
        for ship in HOME_SHIPS {
            if let Some((png, stamp)) = self.thumbnail(ship).await {
                pictures.push(png);
                stamps.push(stamp);
            }
        }
        let path = self.card_path("site", "home", &json!([ships, skins, designs, stamps]));
        if let Some(card) = Self::stored_card(&path).await {
            return Ok(card);
        }
        let freshness = if pictures.len() == HOME_SHIPS.len() {
            Freshness::Final
        } else {
            Freshness::Provisional
        };
        let card = Card {
            eyebrow: "EVE Online ship SKINs".into(),
            title: "Try on every SKIN".into(),
            subtitle: Some("On the real hull, in 3D.".into()),
            tint: Color::constant(DEFAULT_TINT),
            art: Art::Gallery(pictures),
            portrait: None,
            badge: None,
            figure: Some(format!(
                "{} · {} · {}",
                plural(ships, "ship", "ships"),
                plural(skins, "SKIN", "SKINs"),
                plural(designs, "design", "designs"),
            )),
        };
        Ok(self.finish(card, freshness, &path).await)
    }

    /// The Paragon Hub's card: what is on sale, and its best sellers.
    pub async fn hub(&self, pool: &PgPool) -> sqlx::Result<Rendered> {
        let (designs, designers): (i64, i64) = sqlx::query_as(
            "select count(distinct d.id), count(distinct d.creator_id)
             from skinr_designs d
             join skinr_listings l on l.skinr_id = d.id and l.state = $1 and l.expires > now()",
        )
        .bind(LISTED)
        .fetch_one(pool)
        .await?;
        let top: Vec<DesignArt> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "{DESIGN_ART_SELECT}
             join skinr_listings l on l.skinr_id = d.id and l.state = $1 and l.expires > now()
             group by d.id, s.id
             order by count(l.id) desc, sum(l.quantity) desc, d.id
             limit $2"
        )))
        .bind(LISTED)
        .bind(GALLERY_DESIGNS)
        .fetch_all(pool)
        .await?;
        let jobs: Vec<String> = top.iter().map(|design| design.job().key()).collect();
        let path = self.card_path("site", "skinr", &json!([designs, designers, jobs]));
        if let Some(card) = Self::stored_card(&path).await {
            return Ok(card);
        }
        let (pictures, all) = self.gallery(&top).await;
        let card = Card {
            eyebrow: "Paragon Hub".into(),
            title: "SKINR designs".into(),
            subtitle: Some("Player-made, on the real hull.".into()),
            tint: Color::constant(DEFAULT_TINT),
            art: Art::Gallery(pictures),
            portrait: None,
            badge: None,
            figure: Some(format!(
                "{} by {}",
                plural(designs, "design", "designs"),
                plural(designers, "designer", "designers"),
            )),
        };
        let freshness = if all {
            Freshness::Final
        } else {
            Freshness::Provisional
        };
        Ok(self.finish(card, freshness, &path).await)
    }

    /// A ship's card, with its thumbnail; `None` for an unknown ship.
    pub async fn ship(&self, pool: &PgPool, ship_id: i32) -> sqlx::Result<Option<Rendered>> {
        let Some((name, group, race_id, builder, skins, designs)) =
            sqlx::query_as::<_, (String, String, Option<i32>, Option<String>, i64, i64)>(
                "select s.name, g.name, s.race_id, coalesce(f.name, r.name),
                        (select count(*) from ship_skins k where k.ship_id = s.id),
                        (select count(distinct d.id) from skinr_designs d
                         join skinr_listings l on l.skinr_id = d.id and l.state = $2
                              and l.expires > now()
                         where d.ship_type_id = s.id)
                 from ships s
                 join ship_groups g on g.id = s.group_id
                 left join races r on r.id = s.race_id
                 left join factions f on f.id = s.faction_id
                 where s.id = $1",
            )
            .bind(ship_id)
            .bind(LISTED)
            .fetch_optional(pool)
            .await?
        else {
            return Ok(None);
        };
        let thumbnail = self.thumbnail(ship_id).await;
        let eyebrow = match builder {
            Some(builder) => format!("{group} · {builder}"),
            None => group,
        };
        let path = self.card_path(
            "ships",
            &ship_id.to_string(),
            &json!([
                name,
                eyebrow,
                race_id,
                skins,
                designs,
                thumbnail.as_ref().map(|(_, stamp)| stamp)
            ]),
        );
        if let Some(card) = Self::stored_card(&path).await {
            return Ok(Some(card));
        }
        let mut figure = plural(skins, "SKIN", "SKINs");
        if designs > 0 {
            figure = format!(
                "{figure} · {}",
                plural(designs, "SKINR design", "SKINR designs")
            );
        }
        let freshness = if thumbnail.is_some() {
            Freshness::Final
        } else {
            Freshness::Provisional
        };
        let card = Card {
            eyebrow,
            title: name,
            subtitle: None,
            tint: tint(race_id),
            art: thumbnail.map_or(Art::None, |(png, _)| Art::Ship(png)),
            portrait: None,
            badge: None,
            figure: Some(figure),
        };
        Ok(Some(self.finish(card, freshness, &path).await))
    }

    /// An in-game skin's card, the ship drawn in it; `None` for an unknown pair.
    pub async fn skin(
        &self,
        pool: &PgPool,
        ship_id: i32,
        skin_id: i32,
    ) -> sqlx::Result<Option<Rendered>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            ship: String,
            race_id: Option<i32>,
            name: String,
            sof_hull: String,
            sof_faction: String,
            sof_race: String,
            sof_commands: String,
            color_primary: Option<String>,
            color_secondary: Option<String>,
            color_hull: Option<String>,
            icon_colors: Option<String>,
            icon: Option<Vec<u8>>,
        }
        let Some(row) = sqlx::query_as::<_, Row>(
            "select s.name as ship, s.race_id, k.name, s.sof_hull, k.sof_faction, s.sof_race,
                    k.sof_commands, k.color_primary, k.color_secondary, k.color_hull,
                    i.colors as icon_colors, i.png as icon
             from ship_skins k
             join ships s on s.id = k.ship_id
             left join skin_icons i on i.material_id = k.material_id
             where k.ship_id = $1 and k.skin_id = $2",
        )
        .bind(ship_id)
        .bind(skin_id)
        .fetch_optional(pool)
        .await?
        else {
            return Ok(None);
        };
        let job = Job::Dna(dna::livery(
            &row.sof_hull,
            &row.sof_faction,
            &row.sof_race,
            &row.sof_commands,
        ));
        let colors = Bands {
            primary: color(row.color_primary.as_deref()),
            secondary: color(row.color_secondary.as_deref()),
            hull: color(row.color_hull.as_deref()),
        };
        let path = self.card_path(
            "skins",
            &format!("{ship_id}_{skin_id}"),
            &json!([
                row.ship,
                row.name,
                row.race_id,
                row.icon_colors,
                row.icon.is_some(),
                colors,
                job.key()
            ]),
        );
        if let Some(card) = Self::stored_card(&path).await {
            return Ok(Some(card));
        }
        let card = Card {
            eyebrow: format!("{} · SKIN", row.ship),
            title: row.name,
            subtitle: None,
            tint: tint(row.race_id),
            art: Art::None,
            portrait: None,
            badge: Some(skin_badge(row.icon_colors.as_deref(), row.icon, colors)),
            figure: None,
        };
        let own = Own {
            subject: Subject::Skin {
                ship: ship_id,
                skin: skin_id,
            },
            key: dna::livery(
                &row.sof_hull,
                &row.sof_faction,
                &row.sof_race,
                &row.sof_commands,
            ),
        };
        Ok(Some(
            self.with_render(card, &job, &own, ship_id, &path).await,
        ))
    }

    /// Draws `card` with the livery's art (its render, or its own current
    /// thumbnail), or with the bare hull's thumbnail (provisional) while
    /// there is none.
    async fn with_render(
        &self,
        mut card: Card,
        job: &Job,
        own: &Own,
        ship_id: i32,
        path: &std::path::Path,
    ) -> Rendered {
        let render = self.art(job, Some(own)).await;
        let freshness = if render.is_some() {
            Freshness::Final
        } else {
            Freshness::Provisional
        };
        card.art = match render {
            Some(png) => Art::Ship(png),
            None => self
                .thumbnail(ship_id)
                .await
                .map_or(Art::None, |(png, _)| Art::Ship(png)),
        };
        self.finish(card, freshness, path).await
    }

    /// A SKINR design's card, the ship drawn wearing it; `None` for an unknown design.
    pub async fn design(&self, pool: &PgPool, design_id: &str) -> sqlx::Result<Option<Rendered>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            name: String,
            ship: String,
            race_id: Option<i32>,
            creator_name: Option<String>,
            min_price_plex: Option<i64>,
        }
        let Some(row) = sqlx::query_as::<_, Row>(
            "select d.name, s.name as ship, s.race_id, c.name as creator_name,
                    (select min(l.price_plex) from skinr_listings l
                     where l.skinr_id = d.id and l.state = $2 and l.expires > now()) as min_price_plex
             from skinr_designs d
             join ships s on s.id = d.ship_type_id
             left join esi_characters c on c.id = d.creator_id
             where d.id = $1",
        )
        .bind(design_id)
        .bind(LISTED)
        .fetch_optional(pool)
        .await?
        else {
            return Ok(None);
        };
        let art: DesignArt = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "{DESIGN_ART_SELECT} where d.id = $1"
        )))
        .bind(design_id)
        .fetch_one(pool)
        .await?;
        let layout: serde_json::Value = serde_json::from_str(&art.layout).unwrap_or_default();
        let (badge, badge_key) = design_badge(pool, &layout).await?;
        let job = art.job();
        let figure = row.min_price_plex.map_or_else(
            || "Not on sale".to_owned(),
            |price| format!("{} PLEX", thousands(price)),
        );
        let subtitle = row.creator_name.map(|name| format!("by {name}"));
        let path = self.card_path(
            "skinr",
            design_id,
            &json!([
                row.name,
                row.ship,
                row.race_id,
                subtitle,
                figure,
                badge_key,
                job.key()
            ]),
        );
        if let Some(card) = Self::stored_card(&path).await {
            return Ok(Some(card));
        }
        let card = Card {
            eyebrow: format!("{} · SKINR design", row.ship),
            title: row.name,
            subtitle,
            tint: tint(row.race_id),
            art: Art::None,
            portrait: None,
            badge: Some(badge),
            figure: Some(figure),
        };
        Ok(Some(
            self.with_render(card, &job, &art.own(), art.ship_type_id, &path)
                .await,
        ))
    }

    /// A designer's card: portrait, name and their designs on sale; `None`
    /// for a character the site knows nothing of.
    pub async fn designer(
        &self,
        pool: &PgPool,
        character_id: i64,
    ) -> sqlx::Result<Option<Rendered>> {
        let name: Option<String> =
            sqlx::query_scalar("select name from esi_characters where id = $1")
                .bind(character_id)
                .fetch_optional(pool)
                .await?;
        let top: Vec<DesignArt> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "{DESIGN_ART_SELECT}
             join skinr_listings l on l.skinr_id = d.id and l.state = $1 and l.expires > now()
             where d.creator_id = $2
             group by d.id, s.id
             order by count(l.id) desc, d.id"
        )))
        .bind(LISTED)
        .bind(character_id)
        .fetch_all(pool)
        .await?;
        if name.is_none() && top.is_empty() {
            return Ok(None);
        }
        let on_sale = i64::try_from(top.len()).unwrap_or(i64::MAX);
        let shown = &top[..top.len().min(usize::try_from(GALLERY_DESIGNS).unwrap_or(0))];
        let jobs: Vec<String> = shown.iter().map(|design| design.job().key()).collect();
        let path = self.card_path(
            "designers",
            &character_id.to_string(),
            &json!([name, on_sale, jobs]),
        );
        if let Some(card) = Self::stored_card(&path).await {
            return Ok(Some(card));
        }
        let (portrait, (pictures, all)) =
            tokio::join!(self.portrait(character_id), self.gallery(shown));
        let freshness = if all && portrait.is_some() {
            Freshness::Final
        } else {
            Freshness::Provisional
        };
        let card = Card {
            eyebrow: "SKINR designer".into(),
            title: name.unwrap_or_else(|| format!("Designer {character_id}")),
            subtitle: None,
            tint: Color::constant(DEFAULT_TINT),
            art: if pictures.is_empty() {
                Art::None
            } else {
                Art::Gallery(pictures)
            },
            portrait,
            badge: None,
            figure: Some(format!("{} on sale", plural(on_sale, "design", "designs"))),
        };
        Ok(Some(self.finish(card, freshness, &path).await))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_get_thousands_separators() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_250), "1,250");
        assert_eq!(thousands(12_345_678), "12,345,678");
        assert_eq!(thousands(-1_000), "-1,000");
        assert_eq!(plural(1, "design", "designs"), "1 design");
        assert_eq!(plural(2_000, "design", "designs"), "2,000 designs");
    }

    #[test]
    fn races_pick_their_tint() {
        assert_eq!(tint(Some(2)).as_str(), "#e0875a");
        assert_eq!(tint(Some(9999)).as_str(), DEFAULT_TINT);
        assert_eq!(tint(None).as_str(), DEFAULT_TINT);
    }

    #[test]
    fn a_layout_names_the_component_in_each_slot() {
        let layout = json!({"slots": [
            {"id": 3, "configuration": {"nanocoating": {"id": 30}}},
            {"id": 5, "configuration": {"pattern": {"id": 188}}},
            {"id": 1, "configuration": {"nanocoating": {"id": 10}}}
        ]});
        assert_eq!(slot_component(&layout, 1, "nanocoating"), Some(10));
        assert_eq!(slot_component(&layout, 3, "nanocoating"), Some(30));
        assert_eq!(slot_component(&layout, 5, "pattern"), Some(188));
        assert_eq!(slot_component(&layout, 5, "nanocoating"), None);
        assert_eq!(slot_component(&layout, 2, "nanocoating"), None);
        assert_eq!(slot_component(&json!({}), 1, "nanocoating"), None);
    }

    #[test]
    fn a_skin_badge_prefers_its_icon_colours_then_its_icon() {
        let colors = Bands {
            primary: Color::parse("#112233"),
            ..Bands::default()
        };
        let read =
            r##"{"primary":"#aa0000","secondary":"#00aa00","hull":"#0000aa","lights":"#ffffff"}"##;
        let Badge::Hexagon(drawn) = skin_badge(Some(read), Some(b"png".to_vec()), colors.clone())
        else {
            panic!("icon colours draw a hexagon");
        };
        assert_eq!(drawn.bands.hull, Color::parse("#0000aa"));
        assert_eq!(drawn.lights.as_str(), "#ffffff");
        assert_eq!(
            skin_badge(None, Some(b"png".to_vec()), colors.clone()),
            Badge::Icon(b"png".to_vec())
        );
        let Badge::Hexagon(fallback) = skin_badge(Some("junk"), None, colors) else {
            panic!("material colours draw a hexagon");
        };
        assert_eq!(fallback.bands.primary, Color::parse("#112233"));
        assert_eq!(fallback.lights.as_str(), DEFAULT_LIGHTS);
    }
}
