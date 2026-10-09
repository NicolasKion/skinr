//! EVE HTML, the markup in SDE descriptions, following CCP's guide
//! (<https://developers.eveonline.com/docs/guides/eve-html/>).
//!
//! It is not HTML: tags cross (`<b><i></b></i>`), an unclosed tag runs to the
//! end, unknown tags drop but keep their content, exactly four entities are
//! decoded, and whitespace is significant. The output is never markup: either
//! plain text, or [`Segment`]s the frontend renders as text nodes, so nothing
//! from the SDE ever reaches `innerHTML`.

use serde::Serialize;

/// A run of text with one set of styles, or a line break.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Segment {
    Text(Span),
    Break { br: bool },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Span {
    pub text: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
    /// A validated CSS color, `#rrggbbaa`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// The type a `showinfo:` link points at; whether that is worth linking is
    /// the caller's call (only ships have pages here).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_id: Option<i32>,
}

impl Span {
    fn same_style(&self, other: &Span) -> bool {
        self.bold == other.bold
            && self.italic == other.italic
            && self.underline == other.underline
            && self.color == other.color
            && self.type_id == other.type_id
    }
}

/// The styles in force at a point in the text. Tags cross freely, so each
/// toggle is a depth rather than a stack entry: `</b>` undoes one `<b>`
/// wherever it sits, and an unmatched closer changes nothing.
#[derive(Default)]
struct State {
    bold: u32,
    italic: u32,
    underline: u32,
    uppercase: u32,
    /// Colors from `<color>` and `<font color>`; a `<font>` without one pushes
    /// `None` so its `</font>` pops the right entry.
    colors: Vec<Option<String>>,
    links: Vec<Option<i32>>,
}

impl State {
    fn span(&self, text: String) -> Span {
        Span {
            text: if self.uppercase > 0 {
                text.to_uppercase()
            } else {
                text
            },
            bold: self.bold > 0,
            italic: self.italic > 0,
            underline: self.underline > 0,
            color: self.colors.iter().rev().find_map(Clone::clone),
            type_id: self.links.iter().rev().find_map(|link| *link),
        }
    }
}

/// Parses EVE HTML into styled segments, merging neighbours that share styles.
pub fn parse(markup: &str) -> Vec<Segment> {
    let mut segments: Vec<Segment> = Vec::new();
    let mut state = State::default();
    let mut text = String::new();

    let flush = |text: &mut String, state: &State, segments: &mut Vec<Segment>| {
        if text.is_empty() {
            return;
        }
        let span = state.span(std::mem::take(text));
        if let Some(Segment::Text(last)) = segments.last_mut()
            && last.same_style(&span)
        {
            last.text.push_str(&span.text);
            return;
        }
        segments.push(Segment::Text(span));
    };

    let mut rest = markup;
    while !rest.is_empty() {
        let Some(start) = rest.find(['<', '\n', '\r', '\t', '&']) else {
            text.push_str(rest);
            break;
        };
        text.push_str(&rest[..start]);
        rest = &rest[start..];

        match rest.as_bytes()[0] {
            b'\n' => {
                flush(&mut text, &state, &mut segments);
                segments.push(Segment::Break { br: true });
                rest = &rest[1..];
            }
            b'\r' if rest.starts_with("\r\n") => {
                flush(&mut text, &state, &mut segments);
                segments.push(Segment::Break { br: true });
                rest = &rest[2..];
            }
            // A lone carriage return is an ordinary character.
            b'\r' => {
                text.push('\r');
                rest = &rest[1..];
            }
            // A tab is a single space.
            b'\t' => {
                text.push(' ');
                rest = &rest[1..];
            }
            b'&' => {
                let (decoded, length) = entity(rest);
                text.push_str(decoded);
                rest = &rest[length..];
            }
            _ => {
                // '<': a tag up to the next '>', or a literal '<' if none follows.
                let Some(end) = rest.find('>') else {
                    text.push_str(rest);
                    break;
                };
                let tag = &rest[1..end];
                rest = &rest[end + 1..];
                let before = State {
                    colors: state.colors.clone(),
                    links: state.links.clone(),
                    ..state
                };
                let effect = apply_tag(&mut state, tag);
                // Text before a style change keeps the styles it was written in.
                flush(&mut text, &before, &mut segments);
                match effect {
                    TagEffect::Break => segments.push(Segment::Break { br: true }),
                    TagEffect::Column => text.push(' '),
                    TagEffect::Style => {}
                }
            }
        }
    }
    flush(&mut text, &state, &mut segments);
    segments
}

/// The text of EVE HTML with every tag dropped, line breaks as `\n`.
pub fn plain_text(markup: &str) -> String {
    parse(markup)
        .into_iter()
        .map(|segment| match segment {
            Segment::Text(span) => span.text,
            Segment::Break { .. } => "\n".to_owned(),
        })
        .collect()
}

/// The four entities the client decodes, case-insensitively; anything else
/// (`&quot;`, `&#39;`, ...) stays literal. Returns the text and how many bytes
/// of input it consumed.
fn entity(input: &str) -> (&'static str, usize) {
    const ENTITIES: [(&str, &str); 4] = [
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        // A plain space, not U+00A0.
        ("&nbsp;", " "),
    ];
    for (name, value) in ENTITIES {
        if input.len() >= name.len() && input[..name.len()].eq_ignore_ascii_case(name) {
            return (value, name.len());
        }
    }
    ("&", 1)
}

/// What a tag adds to the text besides any style change.
enum TagEffect {
    Style,
    Break,
    /// `<t>`, a column separator; a space keeps the columns apart.
    Column,
}

/// Applies one tag to the state.
fn apply_tag(state: &mut State, tag: &str) -> TagEffect {
    let tag = tag.trim();
    let (closing, body) = match tag.strip_prefix('/') {
        Some(body) => (true, body.trim()),
        None => (false, tag.trim_end_matches('/').trim()),
    };
    let (name, value, attributes) = split_tag(body);
    let name = name.to_ascii_lowercase();

    let toggle = |depth: &mut u32| {
        if closing {
            *depth = depth.saturating_sub(1);
        } else {
            *depth += 1;
        }
    };

    match name.as_str() {
        "br" if !closing => return TagEffect::Break,
        "t" if !closing => return TagEffect::Column,
        "b" => toggle(&mut state.bold),
        "i" => toggle(&mut state.italic),
        "u" => toggle(&mut state.underline),
        "uppercase" => toggle(&mut state.uppercase),
        "color" | "font" => {
            if closing {
                state.colors.pop();
            } else {
                let color = if name == "color" {
                    value.as_deref()
                } else {
                    attribute(&attributes, "color")
                };
                state.colors.push(color.and_then(css_color));
            }
        }
        "a" | "url" => {
            if closing {
                state.links.pop();
            } else {
                let href = if name == "url" {
                    value.as_deref()
                } else {
                    attribute(&attributes, "href")
                };
                state.links.push(href.and_then(showinfo_type));
            }
        }
        // Size, spacing, tooltips, alignment and unknown tags change nothing
        // here; their content stays.
        _ => {}
    }
    TagEffect::Style
}

/// Splits `name=value attr=value attr="quoted"` into the tag name, its own
/// value (the `<color=...>` form) and the remaining attributes.
fn split_tag(body: &str) -> (&str, Option<String>, Vec<(String, String)>) {
    let mut pairs = tokens(body).into_iter();
    let Some((name, value)) = pairs.next() else {
        return ("", None, Vec::new());
    };
    let name = &body[..name.len().min(body.len())];
    let name = name.trim();
    let attributes = pairs
        .filter_map(|(key, value)| Some((key.to_ascii_lowercase(), value?)))
        .collect();
    (name, value, attributes)
}

/// `key=value` tokens, values optionally quoted with ' or ".
fn tokens(body: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let key_start = i;
        while i < chars.len() && chars[i] != '=' && !chars[i].is_whitespace() {
            i += 1;
        }
        let key: String = chars[key_start..i].iter().collect();
        if key.is_empty() {
            i += 1;
            continue;
        }
        let mut value = None;
        if i < chars.len() && chars[i] == '=' {
            i += 1;
            if i < chars.len() && (chars[i] == '"' || chars[i] == '\'') {
                let quote = chars[i];
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != quote {
                    i += 1;
                }
                value = Some(chars[start..i].iter().collect());
                i += 1;
            } else {
                let start = i;
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                value = Some(chars[start..i].iter().collect());
            }
        }
        out.push((key, value));
    }
    out
}

fn attribute<'a>(attributes: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

/// The named colors seen in official data, as `#rrggbb`.
const NAMED_COLORS: [(&str, &str); 8] = [
    ("white", "#ffffff"),
    ("black", "#000000"),
    ("yellow", "#ffff00"),
    ("red", "#ff0000"),
    ("green", "#00ff00"),
    ("blue", "#0000ff"),
    ("orange", "#ffa500"),
    ("gray", "#808080"),
];

/// An EVE color as a CSS `#rrggbbaa`, or `None` when it is not one. EVE puts
/// alpha first (`0xAARRGGBB`, `#AARRGGBB`), sometimes with a stray Python
/// long suffix `L`; six digits mean opaque.
fn css_color(value: &str) -> Option<String> {
    let value = value.trim().trim_matches(['"', '\'']);
    if let Some((_, hex)) = NAMED_COLORS
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(value))
    {
        return Some(format!("{hex}ff"));
    }
    let digits = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .or_else(|| value.strip_prefix('#'))?;
    let digits = digits.trim_end_matches(['L', 'l']);
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let digits = digits.to_ascii_lowercase();
    match digits.len() {
        8 => Some(format!("#{}{}", &digits[2..], &digits[..2])),
        6 => Some(format!("#{digits}ff")),
        _ => None,
    }
}

/// The type id of a `showinfo:<typeID>[//<itemID>]` link; other schemes and
/// malformed ids give `None`. Scheme names compare case-insensitively.
fn showinfo_type(href: &str) -> Option<i32> {
    let href = href.trim().trim_matches(['"', '\'']);
    let (scheme, payload) = href.split_once(':')?;
    if !scheme.eq_ignore_ascii_case("showinfo") {
        return None;
    }
    payload.split("//").next()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(text: &str) -> Segment {
        Segment::Text(Span {
            text: text.into(),
            ..Span::default()
        })
    }

    const BR: Segment = Segment::Break { br: true };

    #[test]
    fn decodes_exactly_four_entities_case_insensitively() {
        assert_eq!(
            plain_text("a &amp; b &LT;c&gt; d&nbsp;e &quot;f&quot; &#39;g&#39; &apos; & h"),
            "a & b <c> d e &quot;f&quot; &#39;g&#39; &apos; & h"
        );
    }

    #[test]
    fn keeps_whitespace_and_breaks_lines_like_the_client() {
        assert_eq!(
            parse("two  spaces\tand tab\r\nnext\rsame<br>last<br/>"),
            vec![
                text("two  spaces and tab"),
                BR,
                text("next\rsame"),
                BR,
                text("last"),
                BR,
            ]
        );
    }

    #[test]
    fn crossing_and_unclosed_tags() {
        assert_eq!(
            parse("<b>bold <i>both</b> italic</i> plain</b> <u>to the end"),
            vec![
                Segment::Text(Span {
                    text: "bold ".into(),
                    bold: true,
                    ..Span::default()
                }),
                Segment::Text(Span {
                    text: "both".into(),
                    bold: true,
                    italic: true,
                    ..Span::default()
                }),
                Segment::Text(Span {
                    text: " italic".into(),
                    italic: true,
                    ..Span::default()
                }),
                text(" plain "),
                Segment::Text(Span {
                    text: "to the end".into(),
                    underline: true,
                    ..Span::default()
                }),
            ]
        );
    }

    #[test]
    fn unknown_tags_drop_but_keep_their_content() {
        assert_eq!(
            plain_text(
                "<hint='tip'>A</hint> <localized hint=\"B\">C</localized> <left>D<t>E <loc>F</loc>"
            ),
            "A C D E F"
        );
        assert_eq!(plain_text("3 < 4"), "3 < 4");
    }

    #[test]
    fn colors_in_every_official_form() {
        assert_eq!(css_color("0xFF33FFFF").as_deref(), Some("#33ffffff"));
        assert_eq!(css_color("#ff3399cc").as_deref(), Some("#3399ccff"));
        assert_eq!(css_color("0xff4cffccL").as_deref(), Some("#4cffccff"));
        assert_eq!(css_color("0x804cffcc").as_deref(), Some("#4cffcc80"));
        assert_eq!(css_color("yellow").as_deref(), Some("#ffff00ff"));
        assert_eq!(css_color("#abcdef").as_deref(), Some("#abcdefff"));
        // Nothing that is not a color reaches CSS.
        assert_eq!(css_color("red;position:fixed"), None);
        assert_eq!(css_color("0xZZ"), None);
        assert_eq!(css_color("#12345"), None);
    }

    #[test]
    fn color_and_font_tags_nest() {
        assert_eq!(
            parse(
                "<color=0xffff0000>red <font size=14 color=\"#ff00ff00\">green</font> red</color> plain <font style=\"position:fixed\">x</font>"
            ),
            vec![
                Segment::Text(Span {
                    text: "red ".into(),
                    color: Some("#ff0000ff".into()),
                    ..Span::default()
                }),
                Segment::Text(Span {
                    text: "green".into(),
                    color: Some("#00ff00ff".into()),
                    ..Span::default()
                }),
                Segment::Text(Span {
                    text: " red".into(),
                    color: Some("#ff0000ff".into()),
                    ..Span::default()
                }),
                text(" plain x"),
            ]
        );
    }

    #[test]
    fn showinfo_links_carry_their_type() {
        assert_eq!(
            parse(
                "<a href=showinfo:603>Merlin</a>, <url=ShowInfo:5//30000142 alt='Jita'>Jita</url>, <a href=\"https://example.com\">web</a>"
            ),
            vec![
                Segment::Text(Span {
                    text: "Merlin".into(),
                    type_id: Some(603),
                    ..Span::default()
                }),
                text(", "),
                Segment::Text(Span {
                    text: "Jita".into(),
                    type_id: Some(5),
                    ..Span::default()
                }),
                text(", web"),
            ]
        );
    }

    #[test]
    fn uppercase_tag() {
        assert_eq!(
            plain_text("<uppercase>caldari</uppercase> state"),
            "CALDARI state"
        );
    }
}
