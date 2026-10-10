//! Reads an in-game skin's colors back off its icon.
//!
//! The client builds a skin icon from four greyscale layers
//! (`skin-icon-{main,secondary,hull,lights}.png`), each multiplied by one of
//! the skin's colors. Dividing the icon by a layer's grey, where only that
//! layer is drawn, gives back that color. The frontend then draws the icon as
//! vectors in those colors, sharp at any size, instead of the 64 px image.

use serde::Serialize;

/// The layers whose colors are read, bottom to top, in the order [`IconColors`] lists them.
pub const LAYERS: [&str; 4] = ["main", "secondary", "hull", "lights"];
/// A layer pixel counts when it is at least this opaque (so it hides the
/// layers below it)…
const OPAQUE: u8 = 240;
/// …and every layer drawn above it is at most this opaque there.
const CLEAR: u8 = 16;
/// Layer greys darker than this say too little about the color to divide by.
const MIN_GREY: u8 = 48;
/// The fewest clean pixels a layer needs for its color to be trusted.
const MIN_PIXELS: usize = 4;

/// A skin's colors as `#rrggbb`: `main` is its primary, `secondary` its
/// secondary, `hull` its hull, `lights` the lit panels.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IconColors {
    pub primary: String,
    pub secondary: String,
    pub hull: String,
    pub lights: String,
}

/// An RGBA image as decoded pixels.
pub struct Rgba {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Rgba {
    pub fn decode(png: &[u8]) -> Option<Self> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder.read_info().ok()?;
        let mut buffer = vec![0; reader.output_buffer_size()?];
        let frame = reader.next_frame(&mut buffer).ok()?;
        buffer.truncate(frame.buffer_size());
        // Normalized to 8 bits per channel; expand what is not RGBA to it.
        let pixels = match frame.color_type {
            png::ColorType::Rgba => buffer,
            png::ColorType::Rgb => buffer
                .chunks(3)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect(),
            png::ColorType::GrayscaleAlpha => buffer
                .chunks(2)
                .flat_map(|p| [p[0], p[0], p[0], p[1]])
                .collect(),
            png::ColorType::Grayscale => buffer.iter().flat_map(|&g| [g, g, g, 255]).collect(),
            png::ColorType::Indexed => return None,
        };
        Some(Self {
            width: frame.width,
            height: frame.height,
            pixels,
        })
    }

    fn pixel(&self, index: usize) -> &[u8] {
        &self.pixels[index * 4..index * 4 + 4]
    }
}

/// The skin's colors from its icon, given the four layers in [`LAYERS`] order.
pub fn icon_colors(icon: &Rgba, layers: &[Rgba; 4]) -> Option<IconColors> {
    if layers
        .iter()
        .any(|layer| layer.width != icon.width || layer.height != icon.height)
    {
        return None;
    }
    let count = (icon.width * icon.height) as usize;
    let color = |which: usize| -> Option<String> {
        let mut estimates: [Vec<f32>; 3] = Default::default();
        for index in 0..count {
            let layer = layers[which].pixel(index);
            let alone = layers[which + 1..]
                .iter()
                .all(|above| above.pixel(index)[3] <= CLEAR);
            if layer[3] < OPAQUE || layer[0] < MIN_GREY || !alone {
                continue;
            }
            let shown = icon.pixel(index);
            let grey = layer[0] as f32;
            for channel in 0..3 {
                estimates[channel].push((shown[channel] as f32 * 255.0 / grey).min(255.0));
            }
        }
        if estimates[0].len() < MIN_PIXELS {
            return None;
        }
        let mut rgb = [0u8; 3];
        for (channel, values) in estimates.iter_mut().enumerate() {
            values.sort_by(f32::total_cmp);
            rgb[channel] = values[values.len() / 2].round() as u8;
        }
        Some(format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]))
    };
    Some(IconColors {
        primary: color(0)?,
        secondary: color(1)?,
        hull: color(2)?,
        lights: color(3)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pixels in a row per layer, enough to pass [`MIN_PIXELS`].
    const RUN: usize = MIN_PIXELS;

    /// A 16×1 image where layer `at` covers its own run of pixels at `grey`.
    fn layer(at: usize, grey: u8) -> Rgba {
        let mut pixels = vec![0u8; 4 * RUN * 4];
        for index in at * RUN..(at + 1) * RUN {
            pixels[index * 4..index * 4 + 4].copy_from_slice(&[grey, grey, grey, 255]);
        }
        Rgba {
            width: (4 * RUN) as u32,
            height: 1,
            pixels,
        }
    }

    /// The icon: each layer's run showing `shown`.
    fn icon(shown: [[u8; 3]; 4]) -> Rgba {
        let mut pixels = Vec::new();
        for color in shown {
            for _ in 0..RUN {
                pixels.extend_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }
        Rgba {
            width: (4 * RUN) as u32,
            height: 1,
            pixels,
        }
    }

    #[test]
    fn divides_the_icon_by_each_layers_grey() {
        // The secondary layer is drawn at half grey, so its color is twice what shows.
        let layers = [layer(0, 255), layer(1, 128), layer(2, 255), layer(3, 255)];
        let colors = icon_colors(
            &icon([[200, 20, 20], [64, 64, 32], [10, 10, 10], [240, 180, 50]]),
            &layers,
        )
        .unwrap();
        assert_eq!(
            colors,
            IconColors {
                primary: "#c81414".into(),
                secondary: "#808040".into(),
                hull: "#0a0a0a".into(),
                lights: "#f0b432".into(),
            }
        );
    }

    #[test]
    fn mismatched_sizes_are_rejected() {
        let small = Rgba {
            width: 1,
            height: 1,
            pixels: vec![0, 0, 0, 0],
        };
        let layers = [layer(0, 255), layer(1, 255), layer(2, 255), layer(3, 255)];
        assert_eq!(icon_colors(&small, &layers), None);
    }
}
