//! The game's SKIN hexagon as vectors, for the cards: the badge the site
//! draws for a livery (`frontend/src/lib/components/skin-hexagon.svelte`,
//! where the shapes and shading are explained; keep the two in step). Drawn
//! on the icon's 64-unit grid and scaled into place.

use std::fmt::Write as _;

use super::svg::{Color, Svg, data_uri};

/// The tint for a colour the SDE leaves out.
const FALLBACK: &str = "#5a5f66";
/// The lights' colour when a skin does not say: the client's default amber.
pub const DEFAULT_LIGHTS: &str = "#f2b234";
/// The grid the hexagon is drawn on.
const GRID: f64 = 64.0;

// The hexagon's corners, as the client's layers cover it.
const TOP_LEFT: &str = "16 5";
const TOP_RIGHT: &str = "48 5";
const RIGHT: &str = "63 32";
const BOTTOM_RIGHT: &str = "48 59";
const BOTTOM_LEFT: &str = "16 59";
const LEFT: &str = "1 32";
// Where the dividers meet the hexagon's sides.
const TOP_EDGE_LEFT: &str = "4.33 26";
const TOP_EDGE_RIGHT: &str = "55.22 18";
const BODY_EDGE_RIGHT: &str = "56.89 21";
const BODY_EDGE_LEFT: &str = "8.22 45";
const WING_EDGE_LEFT: &str = "7.11 43";

/// The top band's lower edge, left to right.
const TOP_DIVIDER: [&str; 9] = [
    TOP_EDGE_LEFT,
    "15 26",
    "18 30",
    "26 30",
    "31 25",
    "33 23",
    "35 21",
    "38 18",
    TOP_EDGE_RIGHT,
];
/// Where the wing ends along the top divider.
const WING_END: usize = 3;
/// The wing's right edge, top to bottom.
const WING_DIVIDER: [&str; 4] = ["26 30", "26 34", "17 43", WING_EDGE_LEFT];
/// The body's upper edge, right to left.
const BODY_DIVIDER: [&str; 6] = [
    BODY_EDGE_RIGHT,
    "47 21",
    "38 30",
    "38 33.5",
    "27 45",
    BODY_EDGE_LEFT,
];

/// Each section's grey down its height (y on the grid, grey 0 to 1).
const SHADING_TOP: [(f64, f64); 6] = [
    (5.0, 0.98),
    (6.5, 0.72),
    (11.0, 0.67),
    (17.0, 0.56),
    (23.0, 0.47),
    (29.0, 0.31),
];
const SHADING_MIDDLE: [(f64, f64); 5] = [
    (16.0, 0.39),
    (19.0, 0.46),
    (25.0, 0.4),
    (34.0, 0.44),
    (46.0, 0.42),
];
const SHADING_BOTTOM: [(f64, f64); 10] = [
    (21.0, 0.94),
    (22.5, 0.69),
    (27.0, 0.65),
    (33.0, 0.54),
    (39.0, 0.48),
    (42.0, 0.43),
    (48.0, 0.44),
    (54.0, 0.46),
    (56.5, 0.5),
    (58.0, 0.57),
];
/// The lights layer's grey: the panels show at this share of their colour.
const LIGHTS_GREY: f64 = 0.75;
/// The client's lit panels down the diagonal (x, y, w, h, brightness).
const LIGHTS: [(f64, f64, f64, f64, f64); 15] = [
    (40.0, 20.0, 7.0, 2.0, 1.0),
    (37.0, 22.0, 8.0, 2.0, 0.8),
    (38.0, 24.0, 1.0, 1.0, 0.5),
    (32.0, 24.0, 2.0, 2.0, 0.7),
    (26.0, 32.0, 3.0, 2.0, 0.6),
    (31.0, 32.0, 2.0, 2.0, 0.9),
    (33.0, 34.0, 3.0, 2.0, 0.75),
    (29.0, 36.0, 6.0, 1.0, 0.55),
    (33.0, 37.0, 1.0, 1.0, 0.4),
    (19.0, 41.0, 2.0, 1.0, 0.5),
    (25.0, 41.0, 2.0, 1.0, 0.65),
    (28.0, 41.0, 2.0, 1.0, 0.45),
    (23.0, 42.0, 4.0, 1.0, 0.8),
    (21.0, 43.0, 7.0, 1.0, 0.9),
    (22.0, 44.0, 5.0, 1.0, 0.6),
];
/// The lights' blur: a glow, and a slightly blurred core.
const LIGHT_GLOW_BLUR: f64 = 0.7;
const LIGHT_CORE_BLUR: f64 = 0.25;
const LIGHT_GLOW_OPACITY: f64 = 0.55;
/// The recessed slot near the body's bottom edge: (path, stroke, opacity).
const SLOT: [(&str, &str, f64); 3] = [
    ("M23 57.5L26.5 53.5H40.5L44 57.5", "#000", 0.4),
    ("M27 55.2L24.6 57.5", "#fff", 0.35),
    ("M27.5 55.2H40", "#fff", 0.1),
];
/// The body's lit rim along the diagonal.
const RIM_OPACITY: f64 = 0.18;
const RIM_WIDTH: f64 = 1.6;
/// The dark seam between sections.
const SEAM_COLOR: &str = "#08090b";
const SEAM_WIDTH: f64 = 0.9;
const SEAM_OPACITY: f64 = 0.8;
const SEAM_BLUR: f64 = 0.35;
/// The panel stripes on the diagonal: period, line height, opacity.
const STRIPE_PERIOD: f64 = 1.6;
const STRIPE_HEIGHT: f64 = 0.5;
const STRIPE_OPACITY: f64 = 0.18;

/// A livery's three bands: top (secondary), middle (hull), bottom (primary).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize)]
pub struct Bands {
    pub primary: Option<Color>,
    pub secondary: Option<Color>,
    pub hull: Option<Color>,
}

/// A pattern laid over the hexagon: its mask (the client's icon, white on
/// black) and its paint.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pattern {
    pub mask: Vec<u8>,
    pub color: Color,
}

/// The hexagon of one livery.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hexagon {
    pub bands: Bands,
    pub lights: Color,
    pub patterns: Vec<Pattern>,
}

/// A livery's badge on a card.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Badge {
    /// Drawn as vectors from its colours.
    Hexagon(Hexagon),
    /// The client's own 64 px icon, for a skin whose colours are unknown.
    Icon(Vec<u8>),
}

fn path(points: &[&str]) -> String {
    format!("M{}Z", points.join("L"))
}

/// One of the hexagon's three sections.
struct Section<'a> {
    name: &'static str,
    /// One or more closed paths.
    paths: Vec<String>,
    tint: &'a str,
    /// Its grey down its height (y on the grid, grey 0 to 1).
    shading: &'static [(f64, f64)],
}

/// The sections, tiling the hexagon: top band, diagonal, bottom.
fn sections(bands: &Bands) -> [Section<'_>; 3] {
    fn tint(color: Option<&Color>) -> &str {
        color.map_or(FALLBACK, Color::as_str)
    }
    let top = [&[TOP_LEFT, TOP_RIGHT][..], &reversed(&TOP_DIVIDER)].concat();
    let middle = [
        &TOP_DIVIDER[WING_END..],
        &BODY_DIVIDER[..],
        &reversed(&WING_DIVIDER)[..WING_DIVIDER.len() - 1],
    ]
    .concat();
    let wing = [&TOP_DIVIDER[..WING_END], &WING_DIVIDER[..], &[LEFT][..]].concat();
    let body = [
        &[BODY_EDGE_RIGHT, RIGHT, BOTTOM_RIGHT, BOTTOM_LEFT][..],
        &reversed(&BODY_DIVIDER)[..BODY_DIVIDER.len() - 1],
    ]
    .concat();
    [
        Section {
            name: "top",
            paths: vec![path(&top)],
            tint: tint(bands.secondary.as_ref().or(bands.primary.as_ref())),
            shading: &SHADING_TOP,
        },
        Section {
            name: "middle",
            paths: vec![path(&middle)],
            tint: tint(bands.hull.as_ref()),
            shading: &SHADING_MIDDLE,
        },
        Section {
            name: "bottom",
            paths: vec![path(&wing), path(&body)],
            tint: tint(bands.primary.as_ref()),
            shading: &SHADING_BOTTOM,
        },
    ]
}

fn reversed<'a>(points: &[&'a str]) -> Vec<&'a str> {
    points.iter().rev().copied().collect()
}

impl Svg {
    /// A livery's badge, `size` px square, its top left at `x`, `y`.
    pub fn badge(&mut self, x: f64, y: f64, size: f64, badge: &Badge) {
        match badge {
            Badge::Icon(png) => self.image(x, y, size, size, png),
            Badge::Hexagon(hexagon) => self.hexagon(x, y, size, hexagon),
        }
    }

    fn hexagon(&mut self, x: f64, y: f64, size: f64, hexagon: &Hexagon) {
        let id = self.id("hex");
        let sections = sections(&hexagon.bands);
        let mut defs = String::new();
        let mut body = String::new();

        for Section {
            name,
            paths,
            shading,
            ..
        } in &sections
        {
            let _ = write!(defs, r#"<clipPath id="{id}-clip-{name}">"#);
            for d in paths {
                let _ = write!(defs, r#"<path d="{d}"/>"#);
            }
            let _ = write!(
                defs,
                r#"</clipPath><linearGradient id="{id}-shade-{name}" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="0" y2="{GRID}">"#
            );
            for (at, grey) in shading.iter() {
                let _ = write!(
                    defs,
                    r##"<stop offset="{}" stop-color="#000" stop-opacity="{}"/>"##,
                    at / GRID,
                    1.0 - grey
                );
            }
            defs.push_str("</linearGradient>");
        }
        let _ = write!(
            defs,
            r##"<pattern id="{id}-stripes" width="{GRID}" height="{STRIPE_PERIOD}" patternUnits="userSpaceOnUse"><rect width="{GRID}" height="{STRIPE_HEIGHT}" fill="#000" fill-opacity="{STRIPE_OPACITY}"/></pattern>"##
        );
        for (index, pattern) in hexagon.patterns.iter().enumerate() {
            let _ = write!(
                defs,
                r#"<mask id="{id}-pattern-{index}" maskUnits="userSpaceOnUse" x="0" y="0" width="{GRID}" height="{GRID}"><image href="{}" x="0" y="0" width="{GRID}" height="{GRID}" preserveAspectRatio="none"/></mask>"#,
                data_uri(&pattern.mask)
            );
        }
        for (name, blur, pad) in [
            ("light-glow", LIGHT_GLOW_BLUR, 0.0),
            ("light-core", LIGHT_CORE_BLUR, 0.0),
            ("seam", SEAM_BLUR, 4.0),
        ] {
            let _ = write!(
                defs,
                r#"<filter id="{id}-{name}" color-interpolation-filters="sRGB" filterUnits="userSpaceOnUse" x="{}" y="{}" width="{}" height="{}"><feGaussianBlur stdDeviation="{blur}"/></filter>"#,
                -pad,
                -pad,
                GRID + 2.0 * pad,
                GRID + 2.0 * pad
            );
        }

        let _ = write!(
            body,
            r#"<g transform="translate({x} {y}) scale({})">"#,
            size / GRID
        );
        // Each section draws only inside itself.
        for Section { name, tint, .. } in &sections {
            let _ = write!(
                body,
                r#"<g clip-path="url(#{id}-clip-{name})"><rect width="{GRID}" height="{GRID}" fill="{tint}"/>"#
            );
            for (index, pattern) in hexagon.patterns.iter().enumerate() {
                let _ = write!(
                    body,
                    r#"<rect width="{GRID}" height="{GRID}" fill="{}" mask="url(#{id}-pattern-{index})"/>"#,
                    pattern.color.as_str()
                );
            }
            let _ = write!(
                body,
                r#"<rect width="{GRID}" height="{GRID}" fill="url(#{id}-shade-{name})"/>"#
            );
            match *name {
                "middle" => lights(&mut body, &id, hexagon.lights.as_str()),
                "bottom" => {
                    let _ = write!(
                        body,
                        r##"<path d="M{}" fill="none" stroke="#fff" stroke-opacity="{RIM_OPACITY}" stroke-width="{RIM_WIDTH}"/><g fill="none" stroke-width="0.8" stroke-linejoin="miter">"##,
                        BODY_DIVIDER.join("L")
                    );
                    for (d, stroke, opacity) in SLOT {
                        let _ = write!(
                            body,
                            r#"<path d="{d}" stroke="{stroke}" stroke-opacity="{opacity}"/>"#
                        );
                    }
                    body.push_str("</g>");
                }
                _ => {}
            }
            body.push_str("</g>");
        }
        // The seams where the sections meet, and the outline.
        let _ = write!(
            body,
            r#"<g fill="none" stroke="{SEAM_COLOR}" stroke-width="{SEAM_WIDTH}" stroke-opacity="{SEAM_OPACITY}" filter="url(#{id}-seam)">"#
        );
        for section in &sections {
            for d in &section.paths {
                let _ = write!(body, r#"<path d="{d}"/>"#);
            }
        }
        body.push_str("</g></g>");

        self.push_defs(&defs);
        self.push_body(&body);
    }
}

/// The diagonal's stripes and lit panels.
fn lights(body: &mut String, id: &str, color: &str) {
    let _ = write!(
        body,
        r#"<rect width="{GRID}" height="{GRID}" fill="url(#{id}-stripes)"/><g fill="{color}"><g filter="url(#{id}-light-glow)" opacity="{LIGHT_GLOW_OPACITY}">"#
    );
    for (x, y, width, height, brightness) in LIGHTS {
        let _ = write!(
            body,
            r#"<rect x="{x}" y="{y}" width="{width}" height="{height}" fill-opacity="{brightness}"/>"#
        );
    }
    let _ = write!(
        body,
        r#"</g><g filter="url(#{id}-light-core)" opacity="{LIGHTS_GREY}">"#
    );
    for (x, y, width, height, brightness) in LIGHTS {
        let _ = write!(
            body,
            r#"<rect x="{}" y="{}" width="{}" height="{}" fill-opacity="{brightness}"/>"#,
            x + 0.25,
            y + 0.2,
            width - 0.5,
            height - 0.4
        );
    }
    body.push_str("</g></g>");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hexagon() -> Hexagon {
        Hexagon {
            bands: Bands {
                primary: Color::parse("#aa2211"),
                secondary: None,
                hull: Color::parse("#334455"),
            },
            lights: Color::constant(DEFAULT_LIGHTS),
            patterns: vec![Pattern {
                mask: b"\x89PNG mask".to_vec(),
                color: Color::constant("#d8d8d8"),
            }],
        }
    }

    #[test]
    fn the_sections_tile_the_hexagon_like_the_site() {
        let bands = hexagon().bands;
        let [top, middle, bottom] = sections(&bands);
        assert_eq!(
            top.paths,
            ["M16 5L48 5L55.22 18L38 18L35 21L33 23L31 25L26 30L18 30L15 26L4.33 26Z"]
        );
        assert_eq!(
            middle.paths,
            [
                "M26 30L31 25L33 23L35 21L38 18L55.22 18L56.89 21L47 21L38 30L38 33.5L27 45L8.22 45L7.11 43L17 43L26 34Z"
            ]
        );
        assert_eq!(
            bottom.paths,
            [
                "M4.33 26L15 26L18 30L26 30L26 34L17 43L7.11 43L1 32Z",
                "M56.89 21L63 32L48 59L16 59L8.22 45L27 45L38 33.5L38 30L47 21Z"
            ]
        );
    }

    #[test]
    fn a_missing_secondary_takes_the_primary() {
        let bands = hexagon().bands;
        let [top, middle, bottom] = sections(&bands);
        assert_eq!(
            (top.tint, middle.tint, bottom.tint),
            ("#aa2211", "#334455", "#aa2211")
        );
        let bare = Bands::default();
        let [top, ..] = sections(&bare);
        assert_eq!(top.tint, FALLBACK);
    }

    #[test]
    fn a_badge_draws_its_patterns_and_lights() {
        let mut svg = Svg::new(100, 100, "#000000");
        svg.badge(10.0, 10.0, 64.0, &Badge::Hexagon(hexagon()));
        let drawn = svg.finish();
        assert_eq!(drawn.matches("data:image/png;base64,").count(), 1);
        assert!(drawn.contains(r##"fill="#d8d8d8" mask="url(#hex"##));
        assert!(drawn.contains(DEFAULT_LIGHTS));
        assert!(drawn.contains("translate(10 10) scale(1)"));
    }

    #[test]
    fn a_skin_without_colours_shows_its_icon() {
        let mut svg = Svg::new(100, 100, "#000000");
        svg.badge(0.0, 0.0, 64.0, &Badge::Icon(b"\x89PNG icon".to_vec()));
        assert_eq!(svg.finish().matches("data:image/png;base64,").count(), 1);
    }
}
