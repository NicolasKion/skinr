//! The one card layout every page shares, in the site's dark look: the
//! mark top left, a portrait or a livery's SKIN badge above a block of type
//! bottom left (eyebrow in the race tint, the name in big display type, a
//! line under it, then a figure), and the art on the right over a glow of
//! the tint.

use super::fonts::Face;
use super::hexagon::Badge;
use super::svg::{Color, Svg, Type, fit};

/// Open Graph's recommended card size.
pub const WIDTH: u32 = 1200;
pub const HEIGHT: u32 = 630;

/// The site's dark tokens (`frontend/src/routes/layout.css`).
const BACKGROUND: &str = "#050506";
const FOREGROUND: &str = "#f5f5f7";
const MUTED: &str = "#86868b";
/// Big type fades from the foreground to this (`--sheen-end`).
const SHEEN_END: &str = "#8e8e93";
/// The hairline ring around portraits (`--glass-edge`, flattened).
const EDGE: &str = "#2a2a2e";
/// The tint's share at the middle and the edge of the glow behind the art
/// (`--race-backdrop-core`, `--race-backdrop-edge`).
const GLOW_CORE: f64 = 0.40;
const GLOW_EDGE: f64 = 0.16;

/// The left margin, and where the type block's eyebrow sits.
const MARGIN: f64 = 72.0;
const EYEBROW_Y: f64 = 330.0;
/// The type block's width beside art, and without any.
const TEXT_WIDTH_BESIDE_ART: f64 = 540.0;
const TEXT_WIDTH_ALONE: f64 = 1056.0;
/// Headline sizes to try, largest first.
const TITLE_SIZES: [f64; 5] = [88.0, 76.0, 66.0, 58.0, 50.0];
/// The mark and wordmark.
const LOGO_SIZE: f64 = 44.0;
const LOGO_Y: f64 = 60.0;
const WORDMARK: &str = "skinr.space";
/// Where the art's glow and single picture sit.
const ART_CENTER: (f64, f64) = (905.0, 330.0);
const ART_SIZE: f64 = 580.0;
const GLOW_RADIUS: f64 = 440.0;
/// A portrait's radius and centre height.
const PORTRAIT_RADIUS: f64 = 64.0;
const PORTRAIT_Y: f64 = 220.0;
/// A livery badge's size; it sits where a portrait would.
const BADGE_SIZE: f64 = 136.0;
/// The badge's grid (64 units) leaves one unit of room left of the hexagon.
const BADGE_INSET: f64 = 1.0 / 64.0;

/// What a card shows on its right.
#[derive(Debug, Clone, Default)]
pub enum Art {
    /// Nothing: the type takes the whole width.
    #[default]
    None,
    /// One ship, a transparent PNG.
    Ship(Vec<u8>),
    /// Up to three ships, the first in front.
    Gallery(Vec<Vec<u8>>),
}

/// A card's content.
#[derive(Debug, Clone)]
pub struct Card {
    pub eyebrow: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub tint: Color,
    pub art: Art,
    pub portrait: Option<Vec<u8>>,
    /// A livery's SKIN badge, where a portrait would be.
    pub badge: Option<Badge>,
    /// A figure along the bottom: a price, a count.
    pub figure: Option<String>,
}

/// The card as an SVG document.
pub fn draw(card: &Card) -> String {
    let mut svg = Svg::new(WIDTH, HEIGHT, BACKGROUND);
    let has_art = !matches!(card.art, Art::None);

    if has_art {
        svg.glow(
            ART_CENTER.0,
            ART_CENTER.1,
            GLOW_RADIUS,
            &card.tint,
            GLOW_CORE,
            GLOW_EDGE,
        );
    } else {
        // A wide wash of the tint, low on the card, so a bare card is not flat.
        svg.glow(
            f64::from(WIDTH) * 0.75,
            f64::from(HEIGHT),
            GLOW_RADIUS * 1.3,
            &card.tint,
            GLOW_CORE * 0.6,
            GLOW_EDGE * 0.6,
        );
    }
    draw_art(&mut svg, &card.art);

    svg.logo(MARGIN, LOGO_Y, LOGO_SIZE, FOREGROUND);
    svg.text(
        MARGIN + LOGO_SIZE + 14.0,
        LOGO_Y + LOGO_SIZE * 0.72,
        WORDMARK,
        Type {
            face: Face::SemiBold,
            size: 28.0,
            fill: FOREGROUND,
            spacing: -0.3,
        },
    );

    if let Some(portrait) = &card.portrait {
        svg.round_image(
            MARGIN + PORTRAIT_RADIUS,
            PORTRAIT_Y,
            PORTRAIT_RADIUS,
            portrait,
            EDGE,
        );
    }
    if let Some(badge) = &card.badge {
        svg.badge(
            MARGIN - BADGE_SIZE * BADGE_INSET,
            PORTRAIT_Y - BADGE_SIZE / 2.0,
            BADGE_SIZE,
            badge,
        );
    }

    let width = if has_art {
        TEXT_WIDTH_BESIDE_ART
    } else {
        TEXT_WIDTH_ALONE
    };
    let eyebrow_spacing = 3.0;
    let (eyebrow, _) = fit(
        &card.eyebrow.to_uppercase(),
        Face::SemiBold,
        eyebrow_spacing,
        width,
        &[20.0],
    );
    svg.text(
        MARGIN,
        EYEBROW_Y,
        &eyebrow,
        Type {
            face: Face::SemiBold,
            size: 20.0,
            fill: card.tint.as_str(),
            spacing: eyebrow_spacing,
        },
    );

    let (title, title_size) = fit(&card.title, Face::DisplayBold, -1.0, width, &TITLE_SIZES);
    let title_y = EYEBROW_Y + 18.0 + title_size * 0.92;
    svg.sheen_text(
        MARGIN,
        title_y,
        &title,
        Type {
            face: Face::DisplayBold,
            size: title_size,
            fill: FOREGROUND,
            spacing: -1.0,
        },
        SHEEN_END,
    );

    if let Some(subtitle) = &card.subtitle {
        let (subtitle, size) = fit(subtitle, Face::Medium, 0.0, width, &[30.0, 26.0]);
        svg.text(
            MARGIN,
            title_y + 54.0,
            &subtitle,
            Type {
                face: Face::Medium,
                size,
                fill: MUTED,
                spacing: 0.0,
            },
        );
    }

    let footer_y = f64::from(HEIGHT) - 72.0;
    if let Some(figure) = &card.figure {
        let (figure, size) = fit(figure, Face::SemiBold, 0.0, width, &[28.0, 24.0]);
        svg.text(
            MARGIN,
            footer_y,
            &figure,
            Type {
                face: Face::SemiBold,
                size,
                fill: FOREGROUND,
                spacing: 0.0,
            },
        );
    }

    svg.finish()
}

fn draw_art(svg: &mut Svg, art: &Art) {
    let (cx, cy) = ART_CENTER;
    match art {
        Art::None => {}
        Art::Ship(png) => {
            let half = ART_SIZE / 2.0;
            svg.image(cx - half, cy - half, ART_SIZE, ART_SIZE, png);
        }
        Art::Gallery(pngs) if pngs.len() == 1 => {
            let half = ART_SIZE / 2.0;
            svg.image(cx - half, cy - half, ART_SIZE, ART_SIZE, &pngs[0]);
        }
        Art::Gallery(pngs) => {
            // A cascade: the first ship large in front, low; the others
            // smaller behind it, up right and up left. Drawn back to front.
            let places = [
                (cx - 20.0, cy + 60.0, ART_SIZE * 0.70),
                (cx + 170.0, cy - 120.0, ART_SIZE * 0.48),
                (cx - 170.0, cy - 130.0, ART_SIZE * 0.40),
            ];
            for ((x, y, size), png) in places.iter().zip(pngs).rev() {
                svg.image(x - size / 2.0, y - size / 2.0, *size, *size, png);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> Card {
        Card {
            eyebrow: "Frigate · Minmatar".into(),
            title: "Rifter <script>".into(),
            subtitle: Some("by Alice & Bob".into()),
            tint: Color::constant("#e0875a"),
            art: Art::None,
            portrait: None,
            badge: None,
            figure: Some("100 PLEX".into()),
        }
    }

    #[test]
    fn a_card_carries_its_text_escaped_and_its_tint() {
        let svg = draw(&card());
        assert!(svg.contains("FRIGATE · MINMATAR"));
        assert!(svg.contains("Rifter &lt;script&gt;"));
        assert!(svg.contains("by Alice &amp; Bob"));
        assert!(svg.contains("#e0875a"));
        assert!(svg.contains("100 PLEX"));
        assert!(svg.contains(WORDMARK));
    }

    #[test]
    fn art_is_embedded() {
        let svg = draw(&Card {
            art: Art::Gallery(vec![b"\x89PNG one".to_vec(), b"\x89PNG two".to_vec()]),
            ..card()
        });
        assert_eq!(svg.matches("data:image/png;base64,").count(), 2);
    }
}
