//! Biome-tinted blocks. Which blocks take the grass/foliage/water colour is hard-coded in vanilla.

use crate::biome::BiomeTint;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Tint {
    #[default]
    None,
    Grass,
    Foliage,
    DryFoliage,
    Water,
    /// Birch and spruce leaves ignore the biome.
    Birch,
    Spruce,
    /// One sRGB colour everywhere (lily pads, stems, redstone wire).
    Fixed([u8; 3]),
}

const GRASS: &[&str] = &["short_grass", "tall_grass", "fern", "large_fern", "tallgrass", "double_plant", "reeds", "sugar_cane", "bush"];
const UNTINTED_LEAVES: &[&str] = &["cherry", "azalea", "pale_oak"];

impl Tint {
    /// sRGB multiplier, `None` for untinted faces. Grass can also depend on the column
    /// ([`BiomeTint::grass_at`]).
    pub fn color(self, biome: &BiomeTint) -> Option<[u8; 3]> {
        match self {
            Tint::None => None,
            Tint::Grass => Some(biome.grass),
            Tint::Foliage => Some(biome.foliage),
            Tint::DryFoliage => Some(biome.dry_foliage),
            Tint::Water => Some(biome.water),
            Tint::Birch => Some([0x80, 0xA7, 0x55]),
            Tint::Spruce => Some([0x61, 0x99, 0x61]),
            Tint::Fixed(color) => Some(color),
        }
    }

    /// What the shader needs to know: 0 untinted, 1 tinted, 3 water (biome opacity).
    pub fn shader_kind(self) -> u32 {
        match self {
            Tint::None => 0,
            Tint::Water => 3,
            _ => 1,
        }
    }
}

/// Tint for each face in [`crate::assets::FACE_NAMES`] order.
pub fn faces(name: &str) -> [Tint; 6] {
    let all = |t| [t; 6];
    match name {
        "water" | "flowing_water" => all(Tint::Water),
        // Sides only tint where their overlay alpha says (see Material::Overlay); the bottom is dirt.
        "grass_block" | "grass" => [Tint::Grass, Tint::Grass, Tint::Grass, Tint::None, Tint::Grass, Tint::Grass],
        n if GRASS.contains(&n) => all(Tint::Grass),
        "birch_leaves" => all(Tint::Birch),
        "spruce_leaves" => all(Tint::Spruce),
        n if n.ends_with("leaves") && !UNTINTED_LEAVES.iter().any(|u| n.contains(u)) => all(Tint::Foliage),
        _ => all(Tint::None),
    }
}
