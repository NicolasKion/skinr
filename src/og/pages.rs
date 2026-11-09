//! The card of each page type: what it shows, read from the database, and
//! what it depends on, which names its file.

use serde_json::json;
use sqlx::PgPool;

use super::cards::{Art, Card};
use super::renders::Job;
use super::svg::Color;
use super::{Freshness, Og, Rendered};
use crate::dna;
use crate::esi::skinr::LISTED;
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
/// SKINR's material slots, in slot order (primary, secondary, detailing, …).
const MATERIAL_SLOTS: [i64; 4] = [1, 2, 3, 4];

fn tint(race_id: Option<i32>) -> Color {
    let hex = race_id
        .and_then(|race| RACE_TINTS.iter().find(|(id, _)| *id == race))
        .map_or(DEFAULT_TINT, |(_, hex)| hex);
    Color::constant(hex)
}

/// Colours from the database, minus any that are not `#rrggbb`.
fn colors<'a>(hexes: impl IntoIterator<Item = Option<&'a str>>) -> Vec<Color> {
    let mut out: Vec<Color> = Vec::new();
    for color in hexes.into_iter().flatten().filter_map(Color::parse) {
        if !out.contains(&color) {
            out.push(color);
        }
    }
    out
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

/// The material component ids of a design's layout, in slot order.
fn material_ids(layout: &serde_json::Value) -> Vec<i32> {
    let slots = layout
        .get("slots")
        .and_then(|slots| slots.as_array())
        .map(Vec::as_slice)
        .unwrap_or_default();
    MATERIAL_SLOTS
        .iter()
        .filter_map(|wanted| {
            slots
                .iter()
                .find(|slot| slot.get("id").and_then(|id| id.as_i64()) == Some(*wanted))?
                .pointer("/configuration/nanocoating/id")?
                .as_i64()
                .and_then(|id| i32::try_from(id).ok())
        })
        .collect()
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
            swatches: Vec::new(),
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
            swatches: Vec::new(),
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
            swatches: Vec::new(),
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
        }
        let Some(row) = sqlx::query_as::<_, Row>(
            "select s.name as ship, s.race_id, k.name, s.sof_hull, k.sof_faction, s.sof_race,
                    k.sof_commands, k.color_primary, k.color_secondary, k.color_hull
             from ship_skins k join ships s on s.id = k.ship_id
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
        let swatches = colors([
            row.color_primary.as_deref(),
            row.color_secondary.as_deref(),
            row.color_hull.as_deref(),
        ]);
        let path = self.card_path(
            "skins",
            &format!("{ship_id}_{skin_id}"),
            &json!([row.ship, row.name, row.race_id, swatches, job.key()]),
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
            swatches,
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
        let ids = material_ids(&layout);
        let found: Vec<(i32, Option<String>)> =
            sqlx::query_as("select id, color from skinr_components where id = any($1)")
                .bind(&ids)
                .fetch_all(pool)
                .await?;
        let swatches = colors(ids.iter().map(|id| {
            found
                .iter()
                .find(|(found, _)| found == id)
                .and_then(|(_, color)| color.as_deref())
        }));
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
                swatches,
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
            swatches,
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
            swatches: Vec::new(),
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
    fn a_layout_names_its_materials_in_slot_order() {
        let layout = json!({"slots": [
            {"id": 3, "configuration": {"nanocoating": {"id": 30}}},
            {"id": 5, "configuration": {"pattern": {"id": 188}}},
            {"id": 1, "configuration": {"nanocoating": {"id": 10}}}
        ]});
        assert_eq!(material_ids(&layout), vec![10, 30]);
        assert_eq!(material_ids(&json!({})), Vec::<i32>::new());
    }

    #[test]
    fn colours_drop_repeats_and_junk() {
        let got = colors([
            Some("#AA0000"),
            None,
            Some("#aa0000"),
            Some("red"),
            Some("#00ff00"),
        ]);
        let got: Vec<&str> = got.iter().map(Color::as_str).collect();
        assert_eq!(got, vec!["#aa0000", "#00ff00"]);
    }
}
