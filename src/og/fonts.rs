//! The Inter faces the cards are set in, compiled into the binary. resvg's
//! system fonts are off, so these are the only faces a card can resolve and
//! it comes out the same on every machine.

use std::sync::{Arc, OnceLock};

use resvg::usvg::fontdb;

/// Small type: eyebrows, captions.
const MEDIUM: &[u8] = include_bytes!("../../assets/fonts/Inter-Medium.ttf");
/// The wordmark and figures.
const SEMI_BOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-SemiBold.ttf");
/// Headlines: Inter's display cut, tighter at large sizes like the site's.
const DISPLAY_BOLD: &[u8] = include_bytes!("../../assets/fonts/InterDisplay-Bold.ttf");

/// Which face a piece of text is set in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Medium,
    SemiBold,
    DisplayBold,
}

/// A registered face: how SVG names it, and its metrics for measuring text.
pub struct LoadedFace {
    family: String,
    weight: u16,
    parsed: ttf_parser::Face<'static>,
}

impl LoadedFace {
    pub fn family(&self) -> &str {
        &self.family
    }

    pub fn weight(&self) -> u16 {
        self.weight
    }

    /// The advance width of `text` at `size` px, plus `spacing` px after each
    /// character (SVG's `letter-spacing`). Kerning is left out: it moves a
    /// fitted line by a pixel or two at most.
    pub fn width(&self, text: &str, size: f64, spacing: f64) -> f64 {
        let scale = size / f64::from(self.parsed.units_per_em());
        text.chars()
            .map(|c| {
                let advance = self
                    .parsed
                    .glyph_index(c)
                    .and_then(|glyph| self.parsed.glyph_hor_advance(glyph))
                    .unwrap_or(0);
                f64::from(advance) * scale + spacing
            })
            .sum()
    }
}

pub struct Fonts {
    database: Arc<fontdb::Database>,
    medium: LoadedFace,
    semi_bold: LoadedFace,
    display_bold: LoadedFace,
}

impl Fonts {
    pub fn database(&self) -> Arc<fontdb::Database> {
        Arc::clone(&self.database)
    }

    pub fn face(&self, face: Face) -> &LoadedFace {
        match face {
            Face::Medium => &self.medium,
            Face::SemiBold => &self.semi_bold,
            Face::DisplayBold => &self.display_bold,
        }
    }
}

/// The faces, loaded once per process.
pub fn fonts() -> &'static Fonts {
    static FONTS: OnceLock<Fonts> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut database = fontdb::Database::new();
        let medium = load(&mut database, MEDIUM);
        let semi_bold = load(&mut database, SEMI_BOLD);
        let display_bold = load(&mut database, DISPLAY_BOLD);
        Fonts {
            database: Arc::new(database),
            medium,
            semi_bold,
            display_bold,
        }
    })
}

/// Registers a face, then reads back the family and weight it was filed under.
fn load(database: &mut fontdb::Database, data: &'static [u8]) -> LoadedFace {
    database.load_font_data(data.to_vec());
    let info = database
        .faces()
        .last()
        .expect("the face just loaded is in the database");
    LoadedFace {
        family: info
            .families
            .first()
            .map(|(family, _)| family.clone())
            .expect("an Inter face names its family"),
        weight: info.weight.0,
        parsed: ttf_parser::Face::parse(data, 0).expect("an Inter face parses"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_face_resolves_to_its_own_family_and_weight() {
        let fonts = fonts();
        let names: Vec<(&str, u16)> = [Face::Medium, Face::SemiBold, Face::DisplayBold]
            .into_iter()
            .map(|face| (fonts.face(face).family(), fonts.face(face).weight()))
            .collect();
        assert_eq!(
            names,
            vec![("Inter", 500), ("Inter", 600), ("Inter Display", 700)]
        );
    }

    #[test]
    fn text_measures_wider_the_longer_and_larger_it_gets() {
        let face = fonts().face(Face::Medium);
        assert_eq!(face.width("", 16.0, 0.0), 0.0);
        assert!(face.width("Rifter", 16.0, 0.0) > face.width("Rift", 16.0, 0.0));
        assert!(face.width("Rifter", 32.0, 0.0) > face.width("Rifter", 16.0, 0.0));
        assert!(face.width("Rifter", 16.0, 2.0) > face.width("Rifter", 16.0, 0.0));
    }
}
