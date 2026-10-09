//! Ship attributes and bonuses from the SDE's dogma files: the values the
//! spec sheet reads (see [`crate::sheet::SHEET_ATTRIBUTES`]) and the typeBonus
//! lines ("5% bonus to ... per level").

use std::collections::{HashMap, HashSet};
use std::io::BufRead;

use serde::Deserialize;

use super::client::Error;
use super::parse::{Localized, read_jsonl};
use super::{BonusKind, ShipAttribute, ShipBonus};
use crate::sheet::SHEET_ATTRIBUTES;

/// An attribute name and the `res:/` path of its client icon.
pub type AttributeIcon = (String, String);

#[derive(Deserialize)]
struct AttributeRow {
    #[serde(rename = "_key")]
    id: i32,
    name: String,
    #[serde(rename = "iconID")]
    icon_id: Option<i32>,
}

#[derive(Deserialize)]
struct IconRow {
    #[serde(rename = "_key")]
    id: i32,
    #[serde(rename = "iconFile")]
    file: Option<String>,
}

#[derive(Deserialize)]
struct UnitRow {
    #[serde(rename = "_key")]
    id: i32,
    #[serde(rename = "displayName", default)]
    display_name: Localized,
}

#[derive(Deserialize)]
struct TypeDogmaRow {
    #[serde(rename = "_key")]
    type_id: i32,
    #[serde(rename = "dogmaAttributes", default)]
    attributes: Vec<AttributeValue>,
}

#[derive(Deserialize)]
struct AttributeValue {
    #[serde(rename = "attributeID")]
    attribute_id: i32,
    value: f64,
}

#[derive(Deserialize)]
struct TypeBonusRow {
    #[serde(rename = "_key")]
    type_id: i32,
    #[serde(default)]
    types: Vec<SkillBonuses>,
    #[serde(rename = "roleBonuses", default)]
    role: Vec<BonusRow>,
    #[serde(rename = "miscBonuses", default)]
    misc: Vec<BonusRow>,
}

#[derive(Deserialize)]
struct SkillBonuses {
    #[serde(rename = "_key")]
    skill_id: i32,
    #[serde(rename = "_value", default)]
    bonuses: Vec<BonusRow>,
}

#[derive(Deserialize)]
struct BonusRow {
    bonus: Option<f64>,
    #[serde(rename = "bonusText", default)]
    text: Localized,
    #[serde(default)]
    importance: i32,
    #[serde(rename = "unitID")]
    unit_id: Option<i32>,
}

/// The sheet's attribute values for the given ships, and the client icon of
/// each sheet attribute (name and `res:/` path).
pub fn ship_attributes(
    ships: &HashSet<i32>,
    dogma_attributes: impl BufRead,
    type_dogma: impl BufRead,
    icons: impl BufRead,
) -> Result<(Vec<ShipAttribute>, Vec<AttributeIcon>), Error> {
    let rows: Vec<AttributeRow> = read_jsonl::<AttributeRow>(dogma_attributes)?
        .into_iter()
        .filter(|row| SHEET_ATTRIBUTES.contains(&row.name.as_str()))
        .collect();
    let files: HashMap<i32, String> = read_jsonl::<IconRow>(icons)?
        .into_iter()
        .filter_map(|row| Some((row.id, row.file?)))
        .collect();
    let mut attribute_icons: Vec<AttributeIcon> = rows
        .iter()
        .filter_map(|row| Some((row.name.clone(), files.get(&row.icon_id?)?.clone())))
        .collect();
    attribute_icons.sort();
    let names: HashMap<i32, String> = rows.into_iter().map(|row| (row.id, row.name)).collect();

    let mut out = Vec::new();
    for row in read_jsonl::<TypeDogmaRow>(type_dogma)? {
        if !ships.contains(&row.type_id) {
            continue;
        }
        for value in row.attributes {
            if let Some(name) = names.get(&value.attribute_id) {
                out.push(ShipAttribute {
                    ship_id: row.type_id,
                    name: name.clone(),
                    value: value.value,
                });
            }
        }
    }
    out.sort_by(|a, b| (a.ship_id, &a.name).cmp(&(b.ship_id, &b.name)));
    Ok((out, attribute_icons))
}

/// The bonus lines of the given ships: skill bonuses grouped by skill (in the
/// SDE's order), then role bonuses, then the rest, each by importance.
pub fn ship_bonuses(
    ships: &HashSet<i32>,
    type_names: &HashMap<i32, String>,
    dogma_units: impl BufRead,
    type_bonus: impl BufRead,
) -> Result<Vec<ShipBonus>, Error> {
    let units: HashMap<i32, String> = read_jsonl::<UnitRow>(dogma_units)?
        .into_iter()
        .filter_map(|row| Some((row.id, row.display_name.en?)))
        .collect();

    let mut out = Vec::new();
    for row in read_jsonl::<TypeBonusRow>(type_bonus)? {
        if !ships.contains(&row.type_id) {
            continue;
        }
        let mut lines: Vec<(BonusKind, Option<i32>, BonusRow)> = Vec::new();
        for skill in row.types {
            let mut bonuses = skill.bonuses;
            bonuses.sort_by_key(|bonus| bonus.importance);
            lines.extend(
                bonuses
                    .into_iter()
                    .map(|bonus| (BonusKind::Skill, Some(skill.skill_id), bonus)),
            );
        }
        for (kind, mut bonuses) in [(BonusKind::Role, row.role), (BonusKind::Misc, row.misc)] {
            bonuses.sort_by_key(|bonus| bonus.importance);
            lines.extend(bonuses.into_iter().map(|bonus| (kind, None, bonus)));
        }

        for (position, (kind, skill_id, bonus)) in lines.into_iter().enumerate() {
            let Some(text) = bonus.text.en.filter(|text| !text.trim().is_empty()) else {
                continue;
            };
            out.push(ShipBonus {
                ship_id: row.type_id,
                position: position as i32,
                kind,
                skill_id,
                skill_name: skill_id.and_then(|id| type_names.get(&id).cloned()),
                bonus: bonus.bonus,
                unit: bonus.unit_id.and_then(|id| units.get(&id).cloned()),
                text,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_sheet_attributes_of_ships() {
        let attributes = concat!(
            r#"{"_key": 14, "name": "hiSlots", "iconID": 293}"#,
            "\n",
            r#"{"_key": 3, "name": "damage"}"#,
        );
        let dogma = concat!(
            r#"{"_key": 603, "dogmaAttributes": [{"attributeID": 14, "value": 3.0}, {"attributeID": 3, "value": 0.0}], "dogmaEffects": []}"#,
            "\n",
            r#"{"_key": 34, "dogmaAttributes": [{"attributeID": 14, "value": 9.0}]}"#,
        );
        let icons = r#"{"_key": 293, "iconFile": "res:/ui/texture/icons/8_64_11.png"}"#;
        let ships = HashSet::from([603]);
        let (values, icons) = ship_attributes(
            &ships,
            attributes.as_bytes(),
            dogma.as_bytes(),
            icons.as_bytes(),
        )
        .unwrap();
        assert_eq!(
            values,
            vec![ShipAttribute {
                ship_id: 603,
                name: "hiSlots".into(),
                value: 3.0
            }]
        );
        assert_eq!(
            icons,
            vec![(
                "hiSlots".to_owned(),
                "res:/ui/texture/icons/8_64_11.png".to_owned()
            )]
        );
    }

    #[test]
    fn bonuses_by_skill_then_role() {
        let units = r#"{"_key": 105, "displayName": {"en": "%"}}"#;
        let bonuses = concat!(
            r#"{"_key": 603, "types": [{"_key": 3330, "_value": ["#,
            r#"{"bonus": 4, "bonusText": {"en": "bonus to shields"}, "importance": 2, "unitID": 105},"#,
            r#"{"bonus": 5, "bonusText": {"en": "bonus to <a href=showinfo:3301>guns</a>"}, "importance": 1, "unitID": 105}"#,
            r#"]}], "roleBonuses": [{"bonusText": {"en": "Can fit a cloak"}, "importance": 1}]}"#,
            "\n",
            r#"{"_key": 999, "roleBonuses": [{"bonusText": {"en": "not a ship"}, "importance": 1}]}"#,
        );
        let names = HashMap::from([(3330, "Caldari Frigate".to_owned())]);
        let lines = ship_bonuses(
            &HashSet::from([603]),
            &names,
            units.as_bytes(),
            bonuses.as_bytes(),
        )
        .unwrap();
        let summary: Vec<_> = lines
            .iter()
            .map(|line| {
                (
                    line.position,
                    line.kind,
                    line.skill_name.as_deref(),
                    line.bonus,
                    line.unit.as_deref(),
                    line.text.as_str(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                (
                    0,
                    BonusKind::Skill,
                    Some("Caldari Frigate"),
                    Some(5.0),
                    Some("%"),
                    "bonus to <a href=showinfo:3301>guns</a>"
                ),
                (
                    1,
                    BonusKind::Skill,
                    Some("Caldari Frigate"),
                    Some(4.0),
                    Some("%"),
                    "bonus to shields"
                ),
                (2, BonusKind::Role, None, None, None, "Can fit a cloak"),
            ]
        );
    }
}
