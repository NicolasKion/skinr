//! The site's readable page URLs, built exactly as the frontend builds them
//! (`frontend/src/lib/urls.ts`): the sitemap and the cards link to the same
//! pages the site does.

use unicode_normalization::UnicodeNormalization;

/// How much of a SKINR design id rides along in its URL.
const DESIGN_ID_CHARS: usize = 12;

/// A name as a URL segment: lowercase letters and digits, dashes between words.
///
/// NFKD, combining marks (U+0300 to U+036F) dropped, lowercased, apostrophes
/// removed, every other run of characters outside `[a-z0-9]` one dash, no
/// dash at either end.
#[must_use]
pub fn slug(name: &str) -> String {
    let lower = name
        .nfkd()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .collect::<String>()
        .to_lowercase();
    let mut slug = String::with_capacity(lower.len());
    let mut gap = false;
    for c in lower.chars().filter(|c| !matches!(c, '\'' | '\u{2019}')) {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if gap && !slug.is_empty() {
                slug.push('-');
            }
            gap = false;
            slug.push(c);
        } else {
            gap = true;
        }
    }
    slug
}

/// `name` slugged, then `-suffix`; only the suffix when the name slugs to nothing.
fn with_suffix(name: &str, suffix: &str) -> String {
    let name = slug(name);
    if name.is_empty() {
        suffix.to_owned()
    } else {
        format!("{name}-{suffix}")
    }
}

/// A ship's page: ship names are unique, so the name is enough.
#[must_use]
pub fn ship_path(ship: &str) -> String {
    format!("/ships/{}", slug(ship))
}

/// An in-game skin's page; skin names are unique per ship.
#[must_use]
pub fn skin_path(ship: &str, skin: &str) -> String {
    format!("{}/skins/{}", ship_path(ship), slug(skin))
}

/// A SKINR design's page: its name and the start of its id, which is what resolves.
#[must_use]
pub fn design_path(ship: &str, design: &str, design_id: &str) -> String {
    let id: String = design_id.chars().take(DESIGN_ID_CHARS).collect();
    format!("{}/skinr/{}", ship_path(ship), with_suffix(design, &id))
}

/// A designer's page on the Paragon Hub: their name, when known, and their id.
#[must_use]
pub fn designer_path(character_id: i64, name: Option<&str>) -> String {
    let id = character_id.to_string();
    format!(
        "/skinr/designer/{}",
        name.map_or_else(|| id.clone(), |name| with_suffix(name, &id))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_become_url_segments() {
        // The cases of frontend/src/lib/urls.test.ts.
        assert_eq!(slug("Kronos"), "kronos");
        assert_eq!(slug("Goru's Shuttle"), "gorus-shuttle");
        assert_eq!(
            slug("Capsule - Genolution 'Auroral' 197-variant"),
            "capsule-genolution-auroral-197-variant"
        );
        assert_eq!(slug("Crème Brûlée"), "creme-brulee");
    }

    #[test]
    fn slugs_trim_and_collapse_everything_else() {
        assert_eq!(slug("  --Megathron  Navy Issue!! "), "megathron-navy-issue");
        assert_eq!(slug("Imperial Navy Slicer’s"), "imperial-navy-slicers");
        assert_eq!(slug("Ｆｕｌｌｗｉｄｔｈ ①"), "fullwidth-1");
        assert_eq!(slug("ßtraße"), "tra-e");
        assert_eq!(slug("鬼"), "");
    }

    #[test]
    fn pages_are_named_like_the_frontend_names_them() {
        assert_eq!(
            ship_path("Megathron Navy Issue"),
            "/ships/megathron-navy-issue"
        );
        assert_eq!(
            skin_path("Rifter", "Rifter Krusual SKIN"),
            "/ships/rifter/skins/rifter-krusual-skin"
        );
        assert_eq!(
            design_path(
                "Covetor",
                "Matte Black Covetor",
                "573b75f1393b5fb651dedda179977ab630d472641c86a3a1ba974b5a56b9d42b"
            ),
            "/ships/covetor/skinr/matte-black-covetor-573b75f1393b"
        );
        assert_eq!(
            designer_path(95_569_887, Some("Nicolas Kion")),
            "/skinr/designer/nicolas-kion-95569887"
        );
        assert_eq!(designer_path(7, None), "/skinr/designer/7");
        assert_eq!(designer_path(7, Some("鬼")), "/skinr/designer/7");
    }
}
