//! Biome-tinted blocks. Which blocks take the grass/foliage/water colour is hard-coded in vanilla.

/// Colour multiplier source; the value is the shader's tint index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Tint {
    #[default]
    None = 0,
    Grass = 1,
    Foliage = 2,
    Water = 3,
}

/// Default plains colours (sRGB): grass, foliage, water (`biomes_client.json` water_surface_color).
pub const DEFAULT_COLORS: [[f32; 3]; 3] = [[0.569, 0.741, 0.349], [0.467, 0.671, 0.184], [0.267, 0.686, 0.961]];
/// `biomes_client.json` default water_surface_transparency, used as the water surface's alpha.
pub const WATER_ALPHA: f32 = 0.65;

const GRASS: &[&str] = &["short_grass", "tall_grass", "fern", "large_fern", "tallgrass", "double_plant", "reeds", "sugar_cane", "bush"];
const UNTINTED_LEAVES: &[&str] = &["cherry", "azalea", "pale_oak"];

/// Tint for each face in [`crate::assets::FACE_NAMES`] order.
pub fn faces(name: &str) -> [Tint; 6] {
    let all = |t| [t; 6];
    match name {
        "water" | "flowing_water" => all(Tint::Water),
        // Sides only tint where their overlay alpha says (see Material::Overlay); the bottom is dirt.
        "grass_block" | "grass" => [Tint::Grass, Tint::Grass, Tint::Grass, Tint::None, Tint::Grass, Tint::Grass],
        n if GRASS.contains(&n) => all(Tint::Grass),
        n if n.ends_with("leaves") && !UNTINTED_LEAVES.iter().any(|u| n.contains(u)) => all(Tint::Foliage),
        _ => all(Tint::None),
    }
}
