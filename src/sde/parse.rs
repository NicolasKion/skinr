//! Parsing the SDE's JSONL files into [`ShipTables`].
//!
//! A ship is a published type in the Ship category whose graphic names a
//! Space Object Factory hull; the graphic's hull, faction and race make up
//! its default DNA. A skin keeps the hull and race, swaps in the faction its
//! material set names (or keeps the ship's) and adds the set's DNA commands:
//! its four materials, its projected pattern and its resource path insert.

use std::collections::{HashMap, HashSet};
use std::io::BufRead;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use super::client::Error;
use super::{Faction, LiveryColors, MetaGroup, Race, Ship, ShipGroup, ShipSkin, ShipTables};

/// The SDE category every ship group belongs to.
pub const SHIP_CATEGORY_ID: i32 = 6;

/// The readers of the files [`super::client::REQUIRED_FILES`] lists.
pub struct SdeFiles<R> {
    pub types: R,
    pub groups: R,
    pub graphics: R,
    pub races: R,
    pub meta_groups: R,
    pub skins: R,
    pub skin_materials: R,
    pub material_sets: R,
    pub factions: R,
    pub type_dogma: R,
    pub dogma_attributes: R,
    pub dogma_units: R,
    pub type_bonus: R,
    pub icons: R,
    pub skinr_components: R,
    pub skinr_component_categories: R,
    pub skinr_slots_to_materials: R,
}

/// A localized SDE string; the viewer keeps English.
#[derive(Debug, Default, Deserialize)]
pub(super) struct Localized {
    #[serde(default)]
    pub(super) en: Option<String>,
}

impl Localized {
    pub(super) fn text(self) -> String {
        self.en.unwrap_or_default()
    }
}

#[derive(Deserialize)]
struct TypeRow {
    #[serde(rename = "_key")]
    id: i32,
    #[serde(default)]
    name: Localized,
    #[serde(default)]
    description: Localized,
    #[serde(rename = "groupID")]
    group_id: i32,
    #[serde(default)]
    published: bool,
    #[serde(rename = "graphicID")]
    graphic_id: Option<i32>,
    #[serde(rename = "raceID")]
    race_id: Option<i32>,
    #[serde(rename = "factionID")]
    faction_id: Option<i32>,
    #[serde(rename = "metaGroupID")]
    meta_group_id: Option<i32>,
    #[serde(rename = "techLevel")]
    tech_level: Option<i32>,
    mass: Option<f64>,
    volume: Option<f64>,
    capacity: Option<f64>,
    radius: Option<f64>,
}

#[derive(Deserialize)]
struct GroupRow {
    #[serde(rename = "_key")]
    id: i32,
    #[serde(default)]
    name: Localized,
    #[serde(rename = "categoryID")]
    category_id: i32,
}

#[derive(Deserialize)]
struct GraphicRow {
    #[serde(rename = "_key")]
    id: i32,
    #[serde(rename = "sofHullName")]
    sof_hull: Option<String>,
    #[serde(rename = "sofFactionName")]
    sof_faction: Option<String>,
    #[serde(rename = "sofRaceName")]
    sof_race: Option<String>,
    /// The material set the hull wears by default (tournament prize ships
    /// and special editions), on top of its faction.
    #[serde(rename = "sofMaterialSetID")]
    sof_material_set_id: Option<i32>,
}

#[derive(Deserialize)]
struct RaceRow {
    #[serde(rename = "_key")]
    id: i32,
    #[serde(default)]
    name: Localized,
    #[serde(default)]
    description: Localized,
}

#[derive(Deserialize)]
struct FactionRow {
    #[serde(rename = "_key")]
    id: i32,
    #[serde(default)]
    name: Localized,
    #[serde(rename = "flatLogo")]
    flat_logo: Option<String>,
    #[serde(default)]
    description: Localized,
    #[serde(rename = "shortDescription", default)]
    short_description: Localized,
    #[serde(rename = "memberRaces", default)]
    member_races: Vec<i32>,
    #[serde(rename = "militiaCorporationID")]
    militia_corporation_id: Option<i64>,
}

#[derive(Deserialize)]
struct Rgb {
    r: f64,
    g: f64,
    b: f64,
}

impl Rgb {
    fn hex(&self) -> String {
        let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!(
            "#{:02x}{:02x}{:02x}",
            channel(self.r),
            channel(self.g),
            channel(self.b)
        )
    }
}

#[derive(Deserialize)]
struct MetaGroupRow {
    #[serde(rename = "_key")]
    id: i32,
    #[serde(default)]
    name: Localized,
    color: Option<Rgb>,
}

#[derive(Deserialize)]
struct SkinRow {
    #[serde(rename = "_key")]
    id: i32,
    #[serde(rename = "skinMaterialID")]
    skin_material_id: i32,
    #[serde(default)]
    types: Vec<i32>,
    #[serde(rename = "visibleTranquility", default)]
    visible_tranquility: bool,
}

#[derive(Deserialize)]
struct SkinMaterialRow {
    #[serde(rename = "_key")]
    id: i32,
    #[serde(rename = "displayName", default)]
    display_name: Localized,
    #[serde(rename = "materialSetID")]
    material_set_id: Option<i32>,
}

#[derive(Deserialize)]
struct MaterialSetRow {
    #[serde(rename = "_key")]
    id: i32,
    #[serde(rename = "sofFactionName")]
    sof_faction: Option<String>,
    #[serde(rename = "colorPrimary")]
    color_primary: Option<Rgb>,
    #[serde(rename = "colorSecondary")]
    color_secondary: Option<Rgb>,
    #[serde(rename = "colorHull")]
    color_hull: Option<Rgb>,
    material1: Option<String>,
    material2: Option<String>,
    material3: Option<String>,
    material4: Option<String>,
    #[serde(rename = "sofPatternName")]
    pattern: Option<String>,
    custommaterial1: Option<String>,
    custommaterial2: Option<String>,
    #[serde(rename = "resPathInsert")]
    res_path_insert: Option<String>,
}

/// The SOF's word for an unset material, pattern material or path insert.
const SOF_NONE: &str = "none";

/// A name the set gives, or `None` when it leaves it unset: missing, empty,
/// or spelled `none` in any case (the SDE writes both `none` and `None`).
fn set_name(name: Option<&String>) -> Option<&str> {
    name.map(String::as_str)
        .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case(SOF_NONE))
}

/// A name the SOF should use, or [`SOF_NONE`] when the set leaves it unset.
fn sof_name(name: Option<&String>) -> &str {
    set_name(name).unwrap_or(SOF_NONE)
}

impl MaterialSetRow {
    /// The SOF DNA commands that dress a hull in this set, joined by `:`
    /// (empty when it only swaps the faction), as the client builds them.
    fn sof_commands(&self) -> String {
        let mut commands = Vec::new();
        let materials = [
            &self.material1,
            &self.material2,
            &self.material3,
            &self.material4,
        ]
        .map(|material| sof_name(material.as_ref()));
        if materials.iter().any(|material| *material != SOF_NONE) {
            commands.push(format!("material?{}", materials.join(";")));
        }
        if let Some(pattern) = set_name(self.pattern.as_ref()) {
            commands.push(format!(
                "pattern?{pattern};{};{}",
                sof_name(self.custommaterial1.as_ref()),
                sof_name(self.custommaterial2.as_ref()),
            ));
        }
        let insert = sof_name(self.res_path_insert.as_ref());
        if insert != SOF_NONE {
            commands.push(format!("respathinsert?{insert}"));
        }
        commands.join(":")
    }

    fn colors(&self) -> LiveryColors {
        LiveryColors {
            primary: self.color_primary.as_ref().map(Rgb::hex),
            secondary: self.color_secondary.as_ref().map(Rgb::hex),
            hull: self.color_hull.as_ref().map(Rgb::hex),
        }
    }
}

/// What one material set does to a hull.
struct MaterialLook {
    /// The SOF faction it swaps in; `None` keeps the ship's own.
    faction: Option<String>,
    /// Its DNA commands (see [`MaterialSetRow::sof_commands`]).
    commands: String,
    colors: LiveryColors,
}

/// The material sets by id, plus the colors of each faction (its lowest-id
/// set) for the liveries no skin names.
struct MaterialSets {
    by_id: HashMap<i32, MaterialLook>,
    by_faction: HashMap<String, LiveryColors>,
}

fn read_material_sets(reader: impl BufRead) -> Result<MaterialSets, Error> {
    let mut rows: Vec<MaterialSetRow> = read_jsonl(reader)?;
    rows.sort_by_key(|row| row.id);

    let mut sets = MaterialSets {
        by_id: HashMap::new(),
        by_faction: HashMap::new(),
    };
    for row in rows {
        let colors = row.colors();
        let commands = row.sof_commands();
        if let Some(faction) = &row.sof_faction {
            sets.by_faction
                .entry(faction.clone())
                .or_insert_with(|| colors.clone());
        }
        // A set that neither swaps the faction nor adds a command changes nothing.
        if row.sof_faction.is_none() && commands.is_empty() {
            continue;
        }
        sets.by_id.insert(
            row.id,
            MaterialLook {
                faction: row.sof_faction,
                commands,
                colors,
            },
        );
    }
    Ok(sets)
}

/// Reads one JSONL file, one record per non-empty line.
pub(super) fn read_jsonl<T: DeserializeOwned>(reader: impl BufRead) -> Result<Vec<T>, Error> {
    let mut rows = Vec::new();
    for (number, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        rows.push(
            serde_json::from_str(&line).map_err(|error| format!("line {}: {error}", number + 1))?,
        );
    }
    Ok(rows)
}

pub fn parse<R: BufRead>(files: SdeFiles<R>) -> Result<ShipTables, Error> {
    let groups: Vec<GroupRow> = read_jsonl(files.groups)?;
    let ship_groups: HashMap<i32, String> = groups
        .into_iter()
        .filter(|group| group.category_id == SHIP_CATEGORY_ID)
        .map(|group| (group.id, group.name.text()))
        .collect();

    let graphics: HashMap<i32, GraphicRow> = read_jsonl::<GraphicRow>(files.graphics)?
        .into_iter()
        .map(|graphic| (graphic.id, graphic))
        .collect();

    let material_sets = read_material_sets(files.material_sets)?;

    let type_rows: Vec<TypeRow> = read_jsonl(files.types)?;
    // Every type's name, for the skills bonuses are listed under.
    let type_names: HashMap<i32, String> = type_rows
        .iter()
        .filter_map(|row| Some((row.id, row.name.en.clone()?)))
        .collect();

    let mut ships = Vec::new();
    for row in type_rows {
        if !row.published || !ship_groups.contains_key(&row.group_id) {
            continue;
        }
        // A ship the SOF cannot build has nothing to show.
        let Some(graphic) = row.graphic_id.and_then(|id| graphics.get(&id)) else {
            continue;
        };
        let (Some(hull), Some(faction), Some(race)) =
            (&graphic.sof_hull, &graphic.sof_faction, &graphic.sof_race)
        else {
            continue;
        };
        let look = graphic
            .sof_material_set_id
            .and_then(|id| material_sets.by_id.get(&id));
        let faction = look
            .and_then(|look| look.faction.as_ref())
            .unwrap_or(faction);

        ships.push(Ship {
            id: row.id,
            name: row.name.text(),
            // Kept as the SDE ships it, EVE HTML; the API renders it (eve_html).
            description: row.description.text(),
            group_id: row.group_id,
            race_id: row.race_id,
            faction_id: row.faction_id,
            meta_group_id: row.meta_group_id,
            tech_level: row.tech_level,
            mass: row.mass,
            volume: row.volume,
            capacity: row.capacity,
            radius: row.radius,
            sof_hull: hull.clone(),
            sof_faction: faction.clone(),
            sof_race: race.clone(),
            sof_commands: look.map(|look| look.commands.clone()).unwrap_or_default(),
            colors: look.map_or_else(
                || {
                    material_sets
                        .by_faction
                        .get(faction)
                        .cloned()
                        .unwrap_or_default()
                },
                |look| look.colors.clone(),
            ),
        });
    }
    ships.sort_by_key(|ship| ship.id);

    let used_groups: HashSet<i32> = ships.iter().map(|ship| ship.group_id).collect();
    let mut groups: Vec<ShipGroup> = ship_groups
        .into_iter()
        .filter(|(id, _)| used_groups.contains(id))
        .map(|(id, name)| ShipGroup { id, name })
        .collect();
    groups.sort_by_key(|group| group.id);

    let races = read_jsonl::<RaceRow>(files.races)?
        .into_iter()
        .map(|row| Race {
            id: row.id,
            name: row.name.text(),
            description: row.description.text(),
        })
        .collect();

    let meta_groups = read_jsonl::<MetaGroupRow>(files.meta_groups)?
        .into_iter()
        .map(|row| MetaGroup {
            id: row.id,
            color: row.color.as_ref().map(Rgb::hex),
            name: row.name.text(),
        })
        .collect();

    let mut factions: Vec<Faction> = read_jsonl::<FactionRow>(files.factions)?
        .into_iter()
        .map(|row| Faction {
            id: row.id,
            name: row.name.text(),
            logo: row.flat_logo,
            description: row.description.text(),
            tagline: Some(row.short_description.text()).filter(|text| !text.is_empty()),
            // The four empires are the factions with a militia of their own
            // that one race makes up; the pirates with militias have several.
            empire_of: match (row.militia_corporation_id, row.member_races.as_slice()) {
                (Some(_), [race]) => Some(*race),
                _ => None,
            },
        })
        .collect();
    factions.sort_by_key(|faction| faction.id);

    let skins = parse_skins(&ships, files.skins, files.skin_materials, &material_sets)?;

    let ship_ids: HashSet<i32> = ships.iter().map(|ship| ship.id).collect();
    let (attributes, attribute_icons) = super::dogma::ship_attributes(
        &ship_ids,
        files.dogma_attributes,
        files.type_dogma,
        files.icons,
    )?;
    let bonuses =
        super::dogma::ship_bonuses(&ship_ids, &type_names, files.dogma_units, files.type_bonus)?;

    let (skinr_components, skinr_slot_materials) = super::skinr::parse(
        files.skinr_components,
        files.skinr_component_categories,
        files.skinr_slots_to_materials,
    )?;

    Ok(ShipTables {
        skinr_components,
        skinr_slot_materials,
        attributes,
        attribute_icons,
        bonuses,
        races,
        factions,
        meta_groups,
        groups,
        ships,
        skins,
    })
}

/// The Tranquility skins of each ship that change its look, one per distinct
/// look (Serenity-only twins share their material set).
fn parse_skins<R: BufRead>(
    ships: &[Ship],
    skins: R,
    skin_materials: R,
    material_sets: &MaterialSets,
) -> Result<Vec<ShipSkin>, Error> {
    let defaults: HashMap<i32, (&str, &str)> = ships
        .iter()
        .map(|ship| {
            (
                ship.id,
                (ship.sof_faction.as_str(), ship.sof_commands.as_str()),
            )
        })
        .collect();

    let materials: HashMap<i32, (String, &MaterialLook)> =
        read_jsonl::<SkinMaterialRow>(skin_materials)?
            .into_iter()
            .filter_map(|material| {
                let look = material_sets.by_id.get(&material.material_set_id?)?;
                Some((material.id, (material.display_name.text(), look)))
            })
            .collect();

    let mut skin_rows: Vec<SkinRow> = read_jsonl(skins)?;
    skin_rows.sort_by_key(|skin| skin.id);

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for skin in skin_rows
        .into_iter()
        .filter(|skin| skin.visible_tranquility)
    {
        let Some((name, look)) = materials.get(&skin.skin_material_id) else {
            continue;
        };
        for ship_id in skin.types {
            let Some(&(default_faction, default_commands)) = defaults.get(&ship_id) else {
                continue;
            };
            let faction = look.faction.as_deref().unwrap_or(default_faction);
            // The ship's own look is not a skin.
            if faction == default_faction && look.commands == default_commands {
                continue;
            }
            if !seen.insert((ship_id, faction.to_owned(), look.commands.clone())) {
                continue;
            }
            out.push(ShipSkin {
                ship_id,
                skin_id: skin.id,
                material_id: skin.skin_material_id,
                name: name.clone(),
                sof_faction: faction.to_owned(),
                sof_commands: look.commands.clone(),
                colors: look.colors.clone(),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(types: &'static str, skins: &'static str) -> SdeFiles<&'static [u8]> {
        SdeFiles {
            types: types.as_bytes(),
            groups: concat!(
                r#"{"_key": 25, "name": {"en": "Frigate"}, "categoryID": 6}"#,
                "\n",
                r#"{"_key": 30, "name": {"en": "Titan"}, "categoryID": 6}"#,
                "\n",
                r#"{"_key": 53, "name": {"en": "Energy Weapon"}, "categoryID": 7}"#,
            )
            .as_bytes(),
            graphics: concat!(
                r#"{"_key": 314, "sofHullName": "cf7_t1", "sofFactionName": "caldaribase", "sofRaceName": "caldari"}"#,
                "\n",
                r#"{"_key": 400, "iconFolder": "res:/x"}"#,
                "\n",
                // A prize hull that wears its own material set.
                r#"{"_key": 315, "sofHullName": "cf7_t1", "sofFactionName": "caldaribase", "sofRaceName": "caldari", "sofMaterialSetID": 104}"#,
            )
            .as_bytes(),
            races: r#"{"_key": 1, "name": {"en": "Caldari"}, "description": {"en": "State"}}"#
                .as_bytes(),
            meta_groups: r#"{"_key": 1, "name": {"en": "Tech I"}, "color": {"r": 1.0, "g": 0.5, "b": 0.0}}"#
                .as_bytes(),
            skins: skins.as_bytes(),
            skin_materials: concat!(
                r#"{"_key": 10, "displayName": {"en": "Nugoeihuvi"}, "materialSetID": 100}"#,
                "\n",
                r#"{"_key": 11, "displayName": {"en": "Caldari"}, "materialSetID": 101}"#,
                "\n",
                r#"{"_key": 12, "displayName": {"en": "Materials only"}, "materialSetID": 102}"#,
                "\n",
                r#"{"_key": 13, "displayName": {"en": "Prize"}, "materialSetID": 104}"#,
            )
            .as_bytes(),
            material_sets: concat!(
                r#"{"_key": 100, "sofFactionName": "noh", "colorPrimary": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0}}"#,
                "\n",
                r#"{"_key": 101, "sofFactionName": "caldaribase", "colorPrimary": {"r": 0.0, "g": 0.0, "b": 1.0, "a": 1.0}, "colorHull": {"r": 0.2, "g": 0.2, "b": 0.2, "a": 1.0}}"#,
                "\n",
                // A later set of the same faction does not override its colors.
                r#"{"_key": 103, "sofFactionName": "caldaribase", "colorPrimary": {"r": 0.0, "g": 1.0, "b": 0.0, "a": 1.0}}"#,
                "\n",
                r#"{"_key": 102, "material1": "black_satin_enamel"}"#,
                "\n",
                r#"{"_key": 104, "sofFactionName": "tournament", "sofPatternName": "prize", "colorPrimary": {"r": 0.0, "g": 1.0, "b": 0.0, "a": 1.0}}"#,
            )
            .as_bytes(),
            type_dogma: b"",
            dogma_attributes: b"",
            dogma_units: b"",
            type_bonus: b"",
            icons: b"",
            skinr_components: b"",
            skinr_component_categories: b"",
            skinr_slots_to_materials: b"",
            factions: concat!(
                r#"{"_key": 500001, "name": {"en": "Caldari State"}, "flatLogo": "caldari_logo", "description": {"en": "Ruled by corporations."}, "shortDescription": {"en": "Duty first."}, "memberRaces": [1], "militiaCorporationID": 1000180}"#,
                "\n",
                r#"{"_key": 500005, "name": {"en": "Jove Empire"}, "memberRaces": [16]}"#,
                "\n",
                r#"{"_key": 500010, "name": {"en": "Guristas Pirates"}, "memberRaces": [1, 8], "militiaCorporationID": 1000437}"#,
            )
            .as_bytes(),
        }
    }

    const MERLIN: &str = r#"{"_key": 603, "name": {"en": "Merlin"}, "description": {"en": "A frigate."}, "groupID": 25, "published": true, "graphicID": 314, "raceID": 1, "factionID": 500001, "metaGroupID": 1, "techLevel": 1, "mass": 997000.0, "volume": 16500.0, "capacity": 150.0, "radius": 39.0}"#;

    #[test]
    fn keeps_published_ships_the_sof_can_build() {
        let types = concat!(
            r#"{"_key": 603, "name": {"en": "Merlin"}, "description": {"en": "A frigate."}, "groupID": 25, "published": true, "graphicID": 314, "raceID": 1, "factionID": 500001, "metaGroupID": 1, "techLevel": 1, "mass": 997000.0, "volume": 16500.0, "capacity": 150.0, "radius": 39.0}"#,
            "\n",
            r#"{"_key": 604, "name": {"en": "Unpublished"}, "groupID": 25, "published": false, "graphicID": 314}"#,
            "\n",
            r#"{"_key": 605, "name": {"en": "No hull"}, "groupID": 25, "published": true, "graphicID": 400}"#,
            "\n",
            r#"{"_key": 606, "name": {"en": "Laser"}, "groupID": 53, "published": true, "graphicID": 314}"#,
        );
        let tables = parse(files(types, "")).unwrap();

        assert_eq!(
            tables.ships,
            vec![Ship {
                id: 603,
                name: "Merlin".into(),
                description: "A frigate.".into(),
                group_id: 25,
                race_id: Some(1),
                faction_id: Some(500001),
                meta_group_id: Some(1),
                tech_level: Some(1),
                mass: Some(997000.0),
                volume: Some(16500.0),
                capacity: Some(150.0),
                radius: Some(39.0),
                sof_hull: "cf7_t1".into(),
                sof_faction: "caldaribase".into(),
                sof_race: "caldari".into(),
                sof_commands: String::new(),
                colors: LiveryColors {
                    primary: Some("#0000ff".into()),
                    secondary: None,
                    hull: Some("#333333".into()),
                },
            }]
        );
        assert_eq!(
            tables.factions,
            vec![
                Faction {
                    id: 500001,
                    name: "Caldari State".into(),
                    logo: Some("caldari_logo".into()),
                    description: "Ruled by corporations.".into(),
                    tagline: Some("Duty first.".into()),
                    empire_of: Some(1),
                },
                // No militia: not an empire, even with one member race.
                Faction {
                    id: 500005,
                    name: "Jove Empire".into(),
                    logo: None,
                    description: String::new(),
                    tagline: None,
                    empire_of: None,
                },
                // A militia, but several member races: a pirate faction.
                Faction {
                    id: 500010,
                    name: "Guristas Pirates".into(),
                    logo: None,
                    description: String::new(),
                    tagline: None,
                    empire_of: None,
                },
            ]
        );
        // Only groups that hold a stored ship.
        assert_eq!(
            tables.groups,
            vec![ShipGroup {
                id: 25,
                name: "Frigate".into()
            }]
        );
        assert_eq!(
            tables.meta_groups,
            vec![MetaGroup {
                id: 1,
                name: "Tech I".into(),
                color: Some("#ff8000".into())
            }]
        );
    }

    #[test]
    fn skins_change_the_look_once_per_ship() {
        let skins = concat!(
            r#"{"_key": 1, "skinMaterialID": 10, "types": [603], "visibleTranquility": true}"#,
            "\n",
            // The Serenity-only twin of the same material set.
            r#"{"_key": 2, "skinMaterialID": 10, "types": [603], "visibleTranquility": false}"#,
            "\n",
            // Same set again on Tranquility: deduplicated.
            r#"{"_key": 3, "skinMaterialID": 10, "types": [603], "visibleTranquility": true}"#,
            "\n",
            // The ship's own faction is not a skin.
            r#"{"_key": 4, "skinMaterialID": 11, "types": [603], "visibleTranquility": true}"#,
            "\n",
            // A material-only set keeps the ship's faction.
            r#"{"_key": 5, "skinMaterialID": 12, "types": [603], "visibleTranquility": true}"#,
            "\n",
            // A type that is not a stored ship.
            r#"{"_key": 6, "skinMaterialID": 10, "types": [999], "visibleTranquility": true}"#,
        );
        let tables = parse(files(MERLIN, skins)).unwrap();

        assert_eq!(
            tables.skins,
            vec![
                ShipSkin {
                    ship_id: 603,
                    skin_id: 1,
                    material_id: 10,
                    name: "Nugoeihuvi".into(),
                    sof_faction: "noh".into(),
                    sof_commands: String::new(),
                    colors: LiveryColors {
                        primary: Some("#ff0000".into()),
                        secondary: None,
                        hull: None,
                    },
                },
                ShipSkin {
                    ship_id: 603,
                    skin_id: 5,
                    material_id: 12,
                    name: "Materials only".into(),
                    sof_faction: "caldaribase".into(),
                    sof_commands: "material?black_satin_enamel;none;none;none".into(),
                    colors: LiveryColors::default(),
                },
            ]
        );
    }

    #[test]
    fn a_hull_wears_its_graphics_material_set() {
        let prize = r#"{"_key": 607, "name": {"en": "Prize"}, "groupID": 25, "published": true, "graphicID": 315}"#;
        let skins = concat!(
            // The hull's own set is its default look, not a skin.
            r#"{"_key": 1, "skinMaterialID": 13, "types": [607], "visibleTranquility": true}"#,
            "\n",
            r#"{"_key": 2, "skinMaterialID": 11, "types": [607], "visibleTranquility": true}"#,
        );
        let tables = parse(files(prize, skins)).unwrap();

        let ship = &tables.ships[0];
        assert_eq!(ship.sof_faction, "tournament");
        assert_eq!(ship.sof_commands, "pattern?prize;none;none");
        assert_eq!(ship.colors.primary.as_deref(), Some("#00ff00"));
        let skins: Vec<_> = tables
            .skins
            .iter()
            .map(|skin| {
                (
                    skin.skin_id,
                    skin.sof_faction.as_str(),
                    skin.sof_commands.as_str(),
                )
            })
            .collect();
        assert_eq!(skins, vec![(2, "caldaribase", "")]);
    }

    #[test]
    fn material_sets_become_dna_commands() {
        let row: MaterialSetRow = serde_json::from_str(
            r#"{"_key": 282, "sofFactionName": "bloodraider_gallente",
                "material1": "grey_tin_metallic", "material2": "black_onyx_metallic",
                "material4": "black_onyx_metallic",
                "sofPatternName": "headhunter_gallente", "custommaterial1": "red_berry_matt",
                "resPathInsert": "none"}"#,
        )
        .unwrap();
        assert_eq!(
            row.sof_commands(),
            "material?grey_tin_metallic;black_onyx_metallic;none;black_onyx_metallic\
             :pattern?headhunter_gallente;red_berry_matt;none"
        );

        let row: MaterialSetRow =
            serde_json::from_str(r#"{"_key": 1, "resPathInsert": "ghost"}"#).unwrap();
        assert_eq!(row.sof_commands(), "respathinsert?ghost");

        // The SDE also spells an unset value `None`.
        let row: MaterialSetRow = serde_json::from_str(
            r#"{"_key": 3, "material1": "None", "material2": "None", "material3": "None",
                "material4": "None", "sofPatternName": "None", "resPathInsert": "None"}"#,
        )
        .unwrap();
        assert_eq!(row.sof_commands(), "");

        let row: MaterialSetRow =
            serde_json::from_str(r#"{"_key": 2, "sofFactionName": "noh"}"#).unwrap();
        assert_eq!(row.sof_commands(), "");
    }
}
