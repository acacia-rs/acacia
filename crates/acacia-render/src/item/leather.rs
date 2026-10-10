//! Leather's dye. The pack's leather textures (worn armour and item icons alike) are TGAs whose
//! alpha is a mask: 0 a hole, 255 grey leather to dye, anything between trim in its own colour.

use std::path::Path;

use image::RgbaImage;

/// Undyed leather.
const UNDYED: [u8; 3] = [0xA0, 0x65, 0x40];

/// Whether `item`'s icon is dyed (`minecraft:leather` itself is not).
pub fn dyeable(item: &str) -> bool {
    item.strip_prefix("minecraft:leather_").is_some_and(|piece| matches!(piece, "helmet" | "chestplate" | "leggings" | "boots" | "horse_armor"))
}

/// Dyes the leather texels with the stack's `dye`, undyed leather's without one, and makes the
/// trim opaque.
pub fn dye(image: &mut RgbaImage, dye: Option<[u8; 3]>) {
    let dye = dye.unwrap_or(UNDYED);
    for p in image.pixels_mut().filter(|p| p.0[3] > 0) {
        if p.0[3] == 255 {
            for (c, dye) in p.0.iter_mut().zip(dye) {
                *c = (u16::from(*c) * u16::from(dye) / 255) as u8;
            }
        }
        p.0[3] = 255;
    }
}

/// An item icon's image; a leather piece's dyed (`leather` is its stack's dye, `Some(None)`
/// undyed, `None` for any other item).
pub fn icon(root: &Path, path: &str, leather: Option<Option<[u8; 3]>>) -> Option<RgbaImage> {
    let file = crate::assets::image_file(root, path)?;
    let mut image = image::open(&file).inspect_err(|e| tracing::warn!(?file, %e, "item icon")).ok()?.to_rgba8();
    if let Some(colour) = leather {
        dye(&mut image, colour);
    }
    Some(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leather_takes_the_stacks_dye_and_trim_keeps_its_colour() {
        let texels = [[200, 200, 200, 255], [90, 60, 30, 1], [7, 7, 7, 0]];
        let dyed = |colour| {
            let mut image = RgbaImage::from_fn(3, 1, |x, _| image::Rgba(texels[x as usize]));
            dye(&mut image, colour);
            image.into_raw()
        };
        assert_eq!(dyed(Some([255, 0, 127])), [200, 0, 99, 255, 90, 60, 30, 255, 7, 7, 7, 0]);
        assert_eq!(dyed(None)[..4], [125, 79, 50, 255]);
    }

    #[test]
    fn armour_pieces_are_dyed_and_the_hide_is_not() {
        assert!(dyeable("minecraft:leather_boots") && dyeable("minecraft:leather_horse_armor"));
        assert!(!dyeable("minecraft:leather") && !dyeable("minecraft:iron_boots"));
    }
}
