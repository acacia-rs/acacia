//! Which Java blocks tint, and with what: Java's `BlockColors`. A model's `tintindex` says which
//! of the block's faces take the colour.

use std::path::Path;

use acacia_render::assets::image::Texture;
use acacia_render::blocks::Tint;

use crate::mapping::JavaState;

const GRASS: &[&str] =
    &["grass_block", "short_grass", "fern", "tall_grass", "large_fern", "potted_fern", "bush", "sugar_cane", "pink_petals", "wildflowers"];
const FOLIAGE: &[&str] = &["oak_leaves", "jungle_leaves", "acacia_leaves", "dark_oak_leaves", "mangrove_leaves", "vine"];
const WATER: &[&str] = &["water", "bubble_column", "water_cauldron"];

/// The colour of tint `index` of a block; all but the flower beds have only index 0.
pub fn of(java: &JavaState, index: i32, dry_foliage: [u8; 3]) -> Tint {
    let number = |key| java.property(key).and_then(|v| v.parse::<u8>().ok()).unwrap_or(0);
    let flower_bed = matches!(java.name.as_str(), "pink_petals" | "wildflowers");
    match java.name.as_str() {
        // Index 0 is the petals, uncoloured; 1 the stems.
        _ if index != i32::from(flower_bed) => Tint::None,
        n if GRASS.contains(&n) => Tint::Grass,
        n if FOLIAGE.contains(&n) => Tint::Foliage,
        n if WATER.contains(&n) => Tint::Water,
        "birch_leaves" => Tint::Birch,
        "spruce_leaves" => Tint::Spruce,
        "lily_pad" => Tint::Fixed([0x20, 0x80, 0x30]),
        "attached_melon_stem" | "attached_pumpkin_stem" => Tint::Fixed([0xE0, 0xC7, 0x1C]),
        "melon_stem" | "pumpkin_stem" => {
            let age = number("age").min(7);
            Tint::Fixed([age * 32, 255 - age * 8, age * 4])
        }
        "redstone_wire" => Tint::Fixed(redstone(number("power"))),
        "leaf_litter" => Tint::Fixed(dry_foliage),
        _ => Tint::None,
    }
}

/// `RedStoneWireBlock`'s colour per signal strength.
fn redstone(power: u8) -> [u8; 3] {
    let f = f32::from(power.min(15)) / 15.0;
    let red = f * 0.6 + if power > 0 { 0.4 } else { 0.3 };
    let green = (f * f * 0.7 - 0.5).clamp(0.0, 1.0);
    let blue = (f * f * 0.6 - 0.7).clamp(0.0, 1.0);
    [red, green, blue].map(|c| (c * 255.0) as u8)
}

/// Leaf litter's colour. Java reads the dry foliage colormap per biome; this is the map near
/// plains (temperature 0.8, downfall 0.4), for every biome.
// TODO: a biome tint kind for dry foliage.
pub fn dry_foliage(assets: &Path) -> [u8; 3] {
    // The 256x256 map nearest-sampled to 16x16: texel (3, 10) is map pixel (48, 160).
    let Ok(map) = Texture::load(&assets.join("textures/colormap/dry_foliage.png"), false) else { return [0x7B, 0x53, 0x34] };
    let at = (10 * 16 + 3) * 4;
    [map.rgba[at], map.rgba[at + 1], map.rgba[at + 2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(name: &str, properties: &[(&str, &str)]) -> JavaState {
        JavaState { name: name.to_owned(), properties: properties.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect() }
    }

    #[test]
    fn blocks_tint_as_java_colours_them() {
        let flowers = |index| of(&state("pink_petals", &[]), index, [1, 2, 3]);
        assert_eq!((flowers(0), flowers(1), flowers(2)), (Tint::None, Tint::Grass, Tint::None));
        assert_eq!(of(&state("vine", &[]), 1, [1, 2, 3]), Tint::None);
        let of = |name: &str, properties: &[(&str, &str)]| of(&state(name, properties), 0, [1, 2, 3]);
        assert_eq!(of("vine", &[]), Tint::Foliage);
        assert_eq!(of("cherry_leaves", &[]), Tint::None);
        assert_eq!(of("leaf_litter", &[]), Tint::Fixed([1, 2, 3]));
        assert_eq!(of("melon_stem", &[("age", "7")]), Tint::Fixed([224, 199, 28]));
        assert_eq!(of("redstone_wire", &[("power", "0")]), Tint::Fixed([76, 0, 0]));
        assert_eq!(of("redstone_wire", &[("power", "15")]), Tint::Fixed([255, 50, 0]));
    }
}
