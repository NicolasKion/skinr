//! The SVG a card is drawn as: a flat list of shapes in paint order, in
//! card pixels, rasterized by resvg.

use std::fmt::Write as _;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

use super::fonts::{Face, fonts};

/// The skinr.space mark (`frontend/src/lib/components/logo.svelte`), in its
/// 32-unit viewBox: the hexagon used as outline and clip.
const LOGO_HEXAGON: &str = "M9 4.5h14L29.5 16 23 27.5H9L2.5 16Z";
/// The mark's upper and lower bands.
const LOGO_UPPER: &str = "M0 0H32V11H21.5L12.5 20H0Z";
const LOGO_LOWER: &str = "M0 22H13.5L22.5 13H32V32H0Z";
/// How far the mark's lower band is dimmed.
const LOGO_LOWER_OPACITY: f64 = 0.42;
/// The logo's viewBox size.
const LOGO_VIEWBOX: f64 = 32.0;
/// The ellipsis a line that does not fit ends in.
const ELLIPSIS: char = '…';

/// A colour as `#rrggbb`, checked, so it can go into an attribute as is.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub struct Color(String);

impl Color {
    /// Parses `#rrggbb` (any case); anything else is `None`.
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        (hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then(|| Self(format!("#{}", hex.to_ascii_lowercase())))
    }

    /// A colour known to be valid at compile time.
    ///
    /// # Panics
    ///
    /// When `text` is not `#rrggbb`: a typo in a constant.
    pub fn constant(text: &str) -> Self {
        Self::parse(text).unwrap_or_else(|| panic!("not a #rrggbb colour: {text}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// How a line of text is set.
#[derive(Debug, Clone, Copy)]
pub struct Type<'a> {
    pub face: Face,
    pub size: f64,
    pub fill: &'a str,
    /// Extra space after each character, in px.
    pub spacing: f64,
}

/// Escapes text for SVG content and attributes.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // Control characters are not allowed in XML at all.
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// An embedded raster as a data URI, typed by its header: character
/// portraits are JPEG, everything else PNG.
pub fn data_uri(bytes: &[u8]) -> String {
    const JPEG_MAGIC: [u8; 3] = [0xFF, 0xD8, 0xFF];
    let mime = if bytes.starts_with(&JPEG_MAGIC) {
        "image/jpeg"
    } else {
        "image/png"
    };
    format!("data:{mime};base64,{}", BASE64.encode(bytes))
}

/// `text` at the largest of `sizes` (largest first) whose width fits in
/// `max_width`; at the smallest, cut and ended with an ellipsis until it
/// fits. Returns the text and the size it is set at.
pub fn fit(text: &str, face: Face, spacing: f64, max_width: f64, sizes: &[f64]) -> (String, f64) {
    let face_metrics = fonts().face(face);
    let smallest = sizes.last().copied().unwrap_or(16.0);
    if let Some(size) = sizes
        .iter()
        .copied()
        .find(|size| face_metrics.width(text, *size, spacing) <= max_width)
    {
        return (text.to_owned(), size);
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let candidate: String = chars
            .iter()
            .collect::<String>()
            .trim_end()
            .chars()
            .chain([ELLIPSIS])
            .collect();
        if face_metrics.width(&candidate, smallest, spacing) <= max_width {
            return (candidate, smallest);
        }
    }
    (ELLIPSIS.to_string(), smallest)
}

/// A card being drawn.
pub struct Svg {
    width: u32,
    height: u32,
    defs: String,
    body: String,
    next_id: usize,
}

impl Svg {
    pub fn new(width: u32, height: u32, background: &str) -> Self {
        let mut svg = Self {
            width,
            height,
            defs: String::new(),
            body: String::new(),
            next_id: 0,
        };
        svg.rect(0.0, 0.0, f64::from(width), f64::from(height), background);
        svg
    }

    fn id(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{prefix}{}", self.next_id)
    }

    pub fn rect(&mut self, x: f64, y: f64, width: f64, height: f64, fill: &str) {
        let _ = write!(
            self.body,
            r#"<rect x="{x}" y="{y}" width="{width}" height="{height}" fill="{fill}"/>"#
        );
    }

    /// A soft round glow of `color`, strongest at the centre.
    pub fn glow(&mut self, cx: f64, cy: f64, radius: f64, color: &Color, core: f64, edge: f64) {
        let id = self.id("glow");
        let color = color.as_str();
        let _ = write!(
            self.defs,
            r#"<radialGradient id="{id}"><stop offset="0" stop-color="{color}" stop-opacity="{core}"/><stop offset="0.55" stop-color="{color}" stop-opacity="{edge}"/><stop offset="1" stop-color="{color}" stop-opacity="0"/></radialGradient>"#
        );
        let _ = write!(
            self.body,
            r#"<circle cx="{cx}" cy="{cy}" r="{radius}" fill="url(#{id})"/>"#
        );
    }

    /// A raster scaled into the box, keeping its aspect.
    pub fn image(&mut self, x: f64, y: f64, width: f64, height: f64, bytes: &[u8]) {
        let _ = write!(
            self.body,
            r#"<image x="{x}" y="{y}" width="{width}" height="{height}" preserveAspectRatio="xMidYMid meet" href="{}"/>"#,
            data_uri(bytes)
        );
    }

    /// A raster cropped into a circle, like the site's avatars.
    pub fn round_image(&mut self, cx: f64, cy: f64, radius: f64, bytes: &[u8], ring: &str) {
        let id = self.id("round");
        let _ = write!(
            self.defs,
            r#"<clipPath id="{id}"><circle cx="{cx}" cy="{cy}" r="{radius}"/></clipPath>"#
        );
        let (x, y, size) = (cx - radius, cy - radius, radius * 2.0);
        let _ = write!(
            self.body,
            r#"<image x="{x}" y="{y}" width="{size}" height="{size}" preserveAspectRatio="xMidYMid slice" clip-path="url(#{id})" href="{}"/><circle cx="{cx}" cy="{cy}" r="{radius}" fill="none" stroke="{ring}" stroke-width="2"/>"#,
            data_uri(bytes)
        );
    }

    pub fn circle(&mut self, cx: f64, cy: f64, radius: f64, fill: &str, ring: &str) {
        let _ = write!(
            self.body,
            r#"<circle cx="{cx}" cy="{cy}" r="{radius}" fill="{fill}" stroke="{ring}" stroke-width="2"/>"#
        );
    }

    /// A line of text with its baseline at `y`, starting at `x`.
    pub fn text(&mut self, x: f64, y: f64, text: &str, style: Type<'_>) {
        let face = fonts().face(style.face);
        let _ = write!(
            self.body,
            r#"<text x="{x}" y="{y}" font-family="{}" font-weight="{}" font-size="{}" letter-spacing="{}" fill="{}">{}</text>"#,
            face.family(),
            face.weight(),
            style.size,
            style.spacing,
            style.fill,
            escape(text)
        );
    }

    /// A headline that fades from `top` to `bottom`, like the site's big type.
    pub fn sheen_text(&mut self, x: f64, y: f64, text: &str, style: Type<'_>, bottom: &str) {
        let id = self.id("sheen");
        let _ = write!(
            self.defs,
            r#"<linearGradient id="{id}" x1="0" y1="0" x2="0" y2="1"><stop offset="0.2" stop-color="{}"/><stop offset="1" stop-color="{bottom}"/></linearGradient>"#,
            style.fill
        );
        let fill = format!("url(#{id})");
        self.text(
            x,
            y,
            text,
            Type {
                fill: &fill,
                ..style
            },
        );
    }

    /// The skinr.space mark, `size` px square, its top left at `x`, `y`.
    pub fn logo(&mut self, x: f64, y: f64, size: f64, color: &str) {
        let id = self.id("logo");
        let scale = size / LOGO_VIEWBOX;
        let _ = write!(
            self.defs,
            r#"<clipPath id="{id}"><path d="{LOGO_HEXAGON}"/></clipPath>"#
        );
        let _ = write!(
            self.body,
            r#"<g transform="translate({x} {y}) scale({scale})"><g clip-path="url(#{id})" fill="{color}"><path d="{LOGO_UPPER}"/><path d="{LOGO_LOWER}" fill-opacity="{LOGO_LOWER_OPACITY}"/></g><path d="{LOGO_HEXAGON}" fill="none" stroke="{color}" stroke-width="1.5" stroke-linejoin="round"/></g>"#
        );
    }

    pub fn finish(self) -> String {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><defs>{}</defs>{}</svg>"#,
            self.defs,
            self.body,
            w = self.width,
            h = self.height
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_hex_colours_pass() {
        assert_eq!(Color::parse("#AA4422").unwrap().as_str(), "#aa4422");
        assert_eq!(Color::parse("aa4422"), None);
        assert_eq!(Color::parse("#aa44"), None);
        assert_eq!(Color::parse("#aa442\"/><script"), None);
    }

    #[test]
    fn text_is_escaped() {
        assert_eq!(
            escape("Tom & \"Jerry\" <3 '\u{7}"),
            "Tom &amp; &quot;Jerry&quot; &lt;3 &apos;"
        );
    }

    #[test]
    fn long_text_shrinks_then_ends_in_an_ellipsis() {
        let sizes = [80.0, 60.0];
        assert_eq!(
            fit("Rifter", Face::DisplayBold, 0.0, 1000.0, &sizes),
            ("Rifter".to_owned(), 80.0)
        );
        let (text, size) = fit(
            "Megathron Navy Issue Quafe Ultra Edition",
            Face::DisplayBold,
            0.0,
            500.0,
            &sizes,
        );
        assert_eq!(size, 60.0);
        assert!(text.ends_with(ELLIPSIS), "{text}");
        assert!(fonts().face(Face::DisplayBold).width(&text, size, 0.0) <= 500.0);
    }

    #[test]
    fn rasters_are_typed_by_their_header() {
        assert!(data_uri(&[0xFF, 0xD8, 0xFF, 0]).starts_with("data:image/jpeg;base64,"));
        assert!(data_uri(b"\x89PNG").starts_with("data:image/png;base64,"));
    }
}
