//! Faction emblems from the game client, for the ship page backdrop.
//!
//! The SDE names each faction's logo (`flatLogo`); the client ships it as a
//! 1024px block-compressed texture under `res:/dx9/model/shared/faction_logos/`,
//! four times the UI icons' 256px. The file names drift from the SDE's
//! (`angel_cartel_logo` is `angel_logo.dds`), so names are matched loosely,
//! and the 256px UI icon is the fallback.

use super::client::{ClientResources, Error};

/// Where the client keeps the full-size faction emblems.
pub const EMBLEM_DIR: &str = "res:/dx9/model/shared/faction_logos/";

/// The UI's own faction icons, the fallback (largest size the client ships).
const UI_ICON_DIR: &str = "res:/ui/texture/eveicon/faction_logos/";
const UI_ICON_SUFFIX: &str = "_256px.png";

/// One faction logo as a PNG.
pub struct Logo {
    pub name: String,
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// The comparable core of a logo name: `the_syndicate_logo`, `syndicate_logo`
/// and `syndicate` all become `syndicate`; `blood_raiders` and `bloodraider`
/// both become `bloodraider`.
pub fn logo_key(name: &str) -> String {
    let name = name.to_ascii_lowercase();
    let name = name.rsplit('/').next().unwrap_or(&name);
    let name = name.split('.').next().unwrap_or(name);
    let name = name.strip_prefix("the_").unwrap_or(name);
    let name = name.strip_suffix("_logo").unwrap_or(name);
    name.replace("_cartel", "")
        .replace('_', "")
        .trim_end_matches('s')
        .to_owned()
}

/// Fetches and converts the logo for each SDE `flatLogo` name.
pub async fn fetch(resources: &ClientResources, names: &[String]) -> Result<Vec<Logo>, Error> {
    let emblems: Vec<(String, String)> = resources
        .paths_under(EMBLEM_DIR)
        .filter(|path| path.ends_with(".dds"))
        .map(|path| (logo_key(path), path.to_owned()))
        .collect();

    let mut logos = Vec::new();
    for name in names {
        let key = logo_key(name);
        if let Some((_, path)) = emblems.iter().find(|(candidate, _)| *candidate == key)
            && let Some(bytes) = resources.read(path).await?
        {
            match dds_to_png(&bytes) {
                Ok((png, width, height)) => {
                    logos.push(Logo {
                        name: name.clone(),
                        png,
                        width,
                        height,
                    });
                    continue;
                }
                Err(error) => println!("could not decode {path} ({error}), trying the UI icon"),
            }
        }

        let icon = format!("{UI_ICON_DIR}{name}{UI_ICON_SUFFIX}");
        match resources.read(&icon).await? {
            Some(png) => logos.push(Logo {
                name: name.clone(),
                png,
                width: 256,
                height: 256,
            }),
            None => println!("no logo for {name} in this client build"),
        }
    }
    Ok(logos)
}

/// The largest mip of a DXT1/DXT5 (BC1/BC3) or BC7 DDS as an RGBA PNG.
pub fn dds_to_png(dds: &[u8]) -> Result<(Vec<u8>, u32, u32), Error> {
    /// The fixed DDS header after the magic, and the DX10 extension.
    const HEADER: usize = 128;
    const DX10_HEADER: usize = 20;
    /// DXGI_FORMAT_BC7_UNORM and its sRGB twin.
    const DXGI_BC7: [u32; 2] = [98, 99];

    if dds.len() < HEADER || &dds[..4] != b"DDS " {
        return Err("not a DDS file".into());
    }
    let word = |offset: usize| u32::from_le_bytes(dds[offset..offset + 4].try_into().unwrap());
    let height = word(12);
    let width = word(16);
    let fourcc = &dds[84..88];
    let (width_px, height_px) = (width as usize, height as usize);

    let mut pixels = vec![0u32; width_px * height_px];
    match fourcc {
        b"DXT1" => texture2ddecoder::decode_bc1(&dds[HEADER..], width_px, height_px, &mut pixels)?,
        b"DXT5" => texture2ddecoder::decode_bc3(&dds[HEADER..], width_px, height_px, &mut pixels)?,
        b"DX10" if dds.len() > HEADER + DX10_HEADER && DXGI_BC7.contains(&word(HEADER)) => {
            texture2ddecoder::decode_bc7(
                &dds[HEADER + DX10_HEADER..],
                width_px,
                height_px,
                &mut pixels,
            )?
        }
        _ => return Err(format!("unsupported DDS format {fourcc:?}").into()),
    }

    // The decoder packs each pixel as little-endian BGRA.
    let rgba: Vec<u8> = pixels
        .iter()
        .flat_map(|pixel| {
            let [b, g, r, a] = pixel.to_le_bytes();
            [r, g, b, a]
        })
        .collect();

    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&rgba)?;
    writer.finish()?;
    Ok((png, width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_names_match_across_the_sde_and_the_client() {
        for (sde, file) in [
            (
                "caldari_logo",
                "res:/dx9/model/shared/faction_logos/caldari_logo.dds",
            ),
            (
                "angel_cartel_logo",
                "res:/dx9/model/shared/faction_logos/angel_logo.dds",
            ),
            (
                "blood_raiders_logo",
                "res:/dx9/model/shared/faction_logos/bloodraider_logo.dds",
            ),
            (
                "mordus_legion_logo",
                "res:/dx9/model/shared/faction_logos/morduslegion_logo.dds",
            ),
            (
                "the_syndicate_logo",
                "res:/dx9/model/shared/faction_logos/syndicate_logo.dds",
            ),
            (
                "guristas_logo",
                "res:/dx9/model/shared/faction_logos/guristas_logo.dds",
            ),
        ] {
            assert_eq!(logo_key(sde), logo_key(file), "{sde} vs {file}");
        }
        assert_ne!(logo_key("amarr_logo"), logo_key("ammatar_logo"));
    }

    /// A 4x4 DXT5 image: one block, opaque red.
    fn red_dxt5() -> Vec<u8> {
        let mut dds = vec![0u8; 128];
        dds[..4].copy_from_slice(b"DDS ");
        dds[12..16].copy_from_slice(&4u32.to_le_bytes());
        dds[16..20].copy_from_slice(&4u32.to_le_bytes());
        dds[84..88].copy_from_slice(b"DXT5");
        // Alpha block: both endpoints 255, every index 0.
        dds.extend_from_slice(&[255, 255, 0, 0, 0, 0, 0, 0]);
        // Color block: both endpoints pure red in RGB565, every index 0.
        dds.extend_from_slice(&[0x00, 0xf8, 0x00, 0xf8, 0, 0, 0, 0]);
        dds
    }

    #[test]
    fn decodes_dxt5_to_rgba_png() {
        let (png, width, height) = dds_to_png(&red_dxt5()).unwrap();
        assert_eq!((width, height), (4, 4));

        let decoder = png::Decoder::new(std::io::Cursor::new(png));
        let mut reader = decoder.read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut pixels).unwrap();
        assert_eq!(&pixels[..4], &[255, 0, 0, 255]);
        assert!(pixels.chunks(4).all(|pixel| pixel == [255, 0, 0, 255]));
    }

    #[test]
    fn rejects_what_it_cannot_decode() {
        assert!(dds_to_png(b"not a dds").is_err());
        let mut dds = red_dxt5();
        dds[84..88].copy_from_slice(b"ATI2");
        assert!(dds_to_png(&dds).is_err());
    }
}
