//! The SDE side of SKINR, the game's player-made ship designs (see the esi-docs
//! guide "Reading SKINR Data"). ESI says which component sits in which slot of
//! a design; these tables say what a component is and, per hull faction,
//! which of Trinity's four hull materials a SKINR material slot paints.

use std::io::BufRead;

use serde::Deserialize;

use super::client::Error;
use super::parse::{Localized, read_jsonl};

/// One SKINR component: a material (a SOF material file) or a pattern (a
/// projection texture), with the icon the game shows for it.
#[derive(Debug, Clone, PartialEq)]
pub struct SkinrComponent {
    pub id: i32,
    pub name: String,
    /// `Material`, `Pattern` or `Metallic` (from `skinrComponentCategories`).
    pub category: String,
    /// `Gloss`, `Matte`, `Metallic`, … when the SDE gives one.
    pub finish: Option<String>,
    pub rarity: Option<i32>,
    /// The SOF material (`.red`) or the pattern texture (`.dds`), as a `res:/` path.
    pub resource_file: String,
    pub icon_file: Option<String>,
    /// What a pattern does past its texture: `repeat`, `clamp-to-edge` or `clamp-to-border`.
    pub projection_u: Option<String>,
    pub projection_v: Option<String>,
}

/// Which Trinity hull material (1 to 4) a SKINR material slot (1 to 4) paints,
/// for hulls of one faction.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotMaterial {
    pub faction_id: i32,
    pub slot_id: i32,
    pub material_id: i32,
}

#[derive(Deserialize)]
struct ComponentRow {
    #[serde(rename = "_key")]
    id: i32,
    #[serde(default)]
    name: Localized,
    category: i32,
    #[serde(default)]
    finish: Option<String>,
    #[serde(default)]
    rarity: Option<i32>,
    #[serde(rename = "resourceFile", default)]
    resource_file: Option<String>,
    #[serde(rename = "iconFile", default)]
    icon_file: Option<String>,
    #[serde(rename = "projectionTypeU", default)]
    projection_u: Option<String>,
    #[serde(rename = "projectionTypeV", default)]
    projection_v: Option<String>,
    #[serde(default)]
    published: bool,
}

#[derive(Deserialize)]
struct CategoryRow {
    #[serde(rename = "_key")]
    id: i32,
    name: String,
}

#[derive(Deserialize)]
struct SlotsToMaterialsRow {
    #[serde(rename = "_key")]
    faction_id: i32,
    #[serde(rename = "_value")]
    slots: Vec<SlotToMaterial>,
}

#[derive(Deserialize)]
struct SlotToMaterial {
    #[serde(rename = "slotID")]
    slot_id: i32,
    #[serde(rename = "materialID")]
    material_id: i32,
}

/// Reads the published components (unpublished ones never appear in designs
/// players can list) and the slot-to-material mapping.
pub fn parse(
    components: impl BufRead,
    categories: impl BufRead,
    slots_to_materials: impl BufRead,
) -> Result<(Vec<SkinrComponent>, Vec<SlotMaterial>), Error> {
    let categories: std::collections::HashMap<i32, String> = read_jsonl::<CategoryRow>(categories)?
        .into_iter()
        .map(|row| (row.id, row.name))
        .collect();
    let components = read_jsonl::<ComponentRow>(components)?
        .into_iter()
        .filter(|row| row.published)
        .filter_map(|row| {
            Some(SkinrComponent {
                id: row.id,
                name: row.name.text(),
                category: categories.get(&row.category)?.clone(),
                finish: row.finish,
                rarity: row.rarity,
                resource_file: row.resource_file?,
                icon_file: row.icon_file,
                projection_u: row.projection_u,
                projection_v: row.projection_v,
            })
        })
        .collect();
    let slots = read_jsonl::<SlotsToMaterialsRow>(slots_to_materials)?
        .into_iter()
        .flat_map(|row| {
            row.slots.into_iter().map(move |slot| SlotMaterial {
                faction_id: row.faction_id,
                slot_id: slot.slot_id,
                material_id: slot.material_id,
            })
        })
        .collect();
    Ok((components, slots))
}

/// How far from the icon's centre pixels are sampled, as a share of its size:
/// inside the sphere, clear of its edge.
const SWATCH_RADIUS: f32 = 0.42;
/// Pixels more transparent than this are background, not sphere.
const SWATCH_MIN_ALPHA: u8 = 200;
/// The share of sampled pixels, by brightness, the swatch averages: the
/// sphere's lit side, without its shadows or the specular highlight.
const SWATCH_BRIGHTNESS_BAND: (f32, f32) = (0.70, 0.97);
/// The icons are lit dimly: the game's white (Stellar White) samples at about
/// #9e9b96. This gain brings that to about #e5e1da and leaves dark materials dark.
const SWATCH_GAIN: f32 = 1.45;

/// A swatch color (`#rrggbb`) for a SKINR material, from the client's icon
/// of it: a sphere rendered in the material, on a transparent background.
pub fn swatch_color(png: &[u8]) -> Option<String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut pixels = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut pixels).ok()?;
    let channels = match frame.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        _ => return None,
    };
    let (width, height) = (frame.width as f32, frame.height as f32);
    let radius = width.min(height) * SWATCH_RADIUS;
    let mut samples: Vec<[u8; 3]> = Vec::new();
    for (index, pixel) in pixels[..frame.buffer_size()].chunks(channels).enumerate() {
        let x = (index as u32 % frame.width) as f32 + 0.5 - width / 2.0;
        let y = (index as u32 / frame.width) as f32 + 0.5 - height / 2.0;
        let opaque = channels == 3 || pixel[3] >= SWATCH_MIN_ALPHA;
        if opaque && x * x + y * y <= radius * radius {
            samples.push([pixel[0], pixel[1], pixel[2]]);
        }
    }
    if samples.is_empty() {
        return None;
    }
    let luminance =
        |p: &[u8; 3]| 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32;
    samples.sort_by(|a, b| luminance(a).total_cmp(&luminance(b)));
    let start = (samples.len() as f32 * SWATCH_BRIGHTNESS_BAND.0) as usize;
    let end = ((samples.len() as f32 * SWATCH_BRIGHTNESS_BAND.1) as usize).max(start + 1);
    let band = &samples[start..end.min(samples.len())];
    let mean = |channel: usize| {
        let average = band.iter().map(|p| p[channel] as f32).sum::<f32>() / band.len() as f32;
        (average * SWATCH_GAIN).round().min(255.0) as u8
    };
    Some(format!("#{:02x}{:02x}{:02x}", mean(0), mean(1), mean(2)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square RGBA PNG: a disc of `disc` on a transparent background.
    fn sphere_icon(size: u32, disc: [u8; 3]) -> Vec<u8> {
        let mut pixels = Vec::new();
        for y in 0..size {
            for x in 0..size {
                let dx = x as f32 + 0.5 - size as f32 / 2.0;
                let dy = y as f32 + 0.5 - size as f32 / 2.0;
                let inside = dx * dx + dy * dy <= (size as f32 * 0.48).powi(2);
                pixels.extend_from_slice(&if inside {
                    [disc[0], disc[1], disc[2], 255]
                } else {
                    [255, 255, 255, 0]
                });
            }
        }
        let mut png = Vec::new();
        let mut encoder = png::Encoder::new(&mut png, size, size);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&pixels).unwrap();
        drop(writer);
        png
    }

    #[test]
    fn swatches_take_the_sphere_not_the_background() {
        assert_eq!(
            swatch_color(&sphere_icon(32, [200, 40, 30])).as_deref(),
            Some("#ff3a2c")
        );
        assert_eq!(swatch_color(b"not a png"), None);
    }

    #[test]
    fn reads_published_components_and_the_slot_mapping() {
        let components = concat!(
            r#"{"_key": 213, "name": {"en": "Stellar White Matte"}, "category": 1, "finish": "Matte", "rarity": 1, "resourceFile": "res:/dx9/model/SpaceObjectFactory/materials/cosm_bw_matt_000_000_100.red", "iconFile": "res:/UI/x.png", "projectionTypeU": "clamp-to-border", "projectionTypeV": "clamp-to-border", "published": true}"#,
            "\n",
            r#"{"_key": 188, "name": {"en": "Hex Recursion"}, "category": 2, "resourceFile": "res:/texture/projection/cosm_Hex_Hex_2k.dds", "projectionTypeU": "repeat", "projectionTypeV": "clamp-to-edge", "published": true}"#,
            "\n",
            r#"{"_key": 9, "name": {"en": "Hidden"}, "category": 1, "resourceFile": "res:/x.red", "published": false}"#,
        );
        let categories = concat!(
            r#"{"_key": 1, "name": "Material"}"#,
            "\n",
            r#"{"_key": 2, "name": "Pattern"}"#,
        );
        let slots = r#"{"_key": 500001, "_value": [{"materialID": 3, "slotID": 1}, {"materialID": 2, "slotID": 2}]}"#;

        let (components, slots) = parse(
            components.as_bytes(),
            categories.as_bytes(),
            slots.as_bytes(),
        )
        .unwrap();

        assert_eq!(components.len(), 2, "the unpublished component is left out");
        assert_eq!(components[0].category, "Material");
        assert_eq!(components[0].finish.as_deref(), Some("Matte"));
        assert_eq!(components[1].category, "Pattern");
        assert_eq!(components[1].projection_u.as_deref(), Some("repeat"));
        assert_eq!(
            slots,
            vec![
                SlotMaterial {
                    faction_id: 500001,
                    slot_id: 1,
                    material_id: 3
                },
                SlotMaterial {
                    faction_id: 500001,
                    slot_id: 2,
                    material_id: 2
                },
            ]
        );
    }
}
