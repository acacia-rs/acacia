//! The enchantment glint: which items shimmer, and how the glint texture lies and moves over them.
//! See README "Enchantment glint".

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::assets::image_file;

/// Java's default `glintStrength`: the glint texel is scaled by it, squared and added.
pub const STRENGTH: f32 = 0.75;
/// Java's default `glintSpeed` times `MAX_ENCHANTMENT_GLINT_SPEED_MILLIS`.
const SPEED: f64 = 0.5 * 8.0;
/// Java's `TextureTransform` scales: `GLINT_TEXTURING` over atlas UVs, `ARMOR_ENTITY_GLINT_TEXTURING`
/// over the armour texture's own.
const ITEM_SCALE: f32 = 8.0;
const ARMOR_SCALE: f32 = 0.16;
/// Side of Java's item and block atlases in texels, which the item scale applies to. Not dumped
/// from the game: see the README.
const JAVA_ATLAS: f32 = 2048.0;
/// The turn of the glint texture, `rotateZ(π / 18)`.
const TURN: f32 = std::f32::consts::PI / 18.0;

/// What a glinting instance is, which picks the texture and its scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glint {
    /// An item, held or dropped.
    Item,
    /// A worn piece of armour.
    Armor,
}

impl Glint {
    /// What the instance's UVs (0 to 1 over a texture `size` texels big) are scaled by.
    pub fn scale(self, size: [u32; 2]) -> [f32; 2] {
        match self {
            Glint::Item => size.map(|texels| ITEM_SCALE * texels as f32 / JAVA_ATLAS),
            Glint::Armor => [ARMOR_SCALE; 2],
        }
    }
}

/// Which items shimmer without an enchantment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Foil {
    /// Potions, and the items below. Not read from the game: the client decides it.
    #[default]
    Bedrock,
    /// The items Java's `Items` gives `ENCHANTMENT_GLINT_OVERRIDE` (26.3); potions lost theirs.
    Java,
}

impl Foil {
    /// Whether `item` (`minecraft:nether_star`) always shimmers.
    pub fn always(self, item: &str) -> bool {
        let name = item.strip_prefix("minecraft:").unwrap_or(item);
        match (name, self) {
            ("enchanted_golden_apple" | "nether_star" | "enchanted_book" | "experience_bottle", _) => true,
            ("potion" | "splash_potion" | "lingering_potion", Foil::Bedrock) => true,
            ("written_book" | "end_crystal", Foil::Java) => true,
            _ => false,
        }
    }
}

/// How far the glint texture has slid `millis` after the start, in repeats: Java's
/// `setupGlintTexturing`, once across in 27.5 s and once up in 7.5 s.
pub fn scroll(millis: u64) -> [f32; 2] {
    let ticks = (millis as f64 * SPEED) as u64;
    [-((ticks % 110_000) as f32 / 110_000.0), (ticks % 30_000) as f32 / 30_000.0]
}

/// Where a vertex samples the glint texture: scaled, turned, then slid. `gpu/entity.wgsl` and
/// `gpu/ui.wgsl` do the same.
pub fn uv(uv: [f32; 2], scale: [f32; 2], scroll: [f32; 2]) -> [f32; 2] {
    let (x, y) = (uv[0] * scale[0], uv[1] * scale[1]);
    let (sin, cos) = TURN.sin_cos();
    [x * cos - y * sin + scroll[0], x * sin + y * cos + scroll[1]]
}

/// A look's glint textures.
pub struct Images {
    pub item: image::RgbaImage,
    pub armor: image::RgbaImage,
}

impl Images {
    /// Java's `enchanted_glint_item` and `enchanted_glint_armor` where the look's files have
    /// them, else Bedrock's `enchanted_item_glint` and `enchanted_actor_glint`.
    pub fn load(files: &Path) -> Option<Images> {
        let open = |name: &str| Some(image::open(image_file(files, &format!("textures/misc/{name}"))?).ok()?.into_rgba8());
        let pair = |item: &str, armor: &str| Some(Images { item: open(item)?, armor: open(armor)? });
        let bedrock = || pair("enchanted_item_glint", "enchanted_actor_glint").map(|i| Images { item: tinted(i.item), armor: tinted(i.armor) });
        pair("enchanted_glint_item", "enchanted_glint_armor").or_else(bedrock)
    }
}

/// Bedrock's glint textures are grey and its client colours them; the colour is not in the pack.
/// Assumed: the purple Java multiplied its own grey glint by before 1.15.
const BEDROCK_TINT: [u16; 3] = [0x80, 0x40, 0xCC];

fn tinted(mut image: image::RgbaImage) -> image::RgbaImage {
    for pixel in image.pixels_mut() {
        for (channel, tint) in pixel.0.iter_mut().zip(BEDROCK_TINT) {
            *channel = (u16::from(*channel) * tint / 255) as u8;
        }
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_texture_slides_as_javas_does() {
        assert_eq!(scroll(0), [0.0, 0.0]);
        // 1000 ms are 4000 of Java's ticks.
        assert_eq!(scroll(1000), [-4000.0 / 110_000.0, 4000.0 / 30_000.0]);
        // Both wrap: the way up every 7.5 s, the way across every 27.5 s.
        assert_eq!(scroll(7500)[1], 0.0);
        assert_eq!(scroll(27_500)[0], 0.0);
        assert!((scroll(27_499)[0] + 1.0).abs() < 1e-4);
    }

    #[test]
    fn uvs_are_scaled_turned_then_slid() {
        // new Matrix4f().translation(-0.25, 0.5, 0).rotateZ(π/18).scale(8) applied to (1, 0).
        let [u, v] = uv([1.0, 0.0], [8.0, 8.0], [-0.25, 0.5]);
        assert!((u - (8.0 * TURN.cos() - 0.25)).abs() < 1e-6 && (v - (8.0 * TURN.sin() + 0.5)).abs() < 1e-6, "{u} {v}");
        // A 16-texel sprite spans a sixteenth of the glint texture; armour 0.16 of it.
        assert_eq!(Glint::Item.scale([16, 16]), [0.0625, 0.0625]);
        assert_eq!(Glint::Item.scale([16, 32]), [0.0625, 0.125]);
        assert_eq!(Glint::Armor.scale([64, 32]), [0.16, 0.16]);
    }

    #[test]
    fn bedrocks_grey_glint_is_coloured() {
        let grey = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255]));
        assert_eq!(tinted(grey).get_pixel(0, 0).0, [0x80, 0x40, 0xCC, 255]);
    }

    #[test]
    fn each_game_has_its_own_items_that_always_shimmer() {
        for both in ["minecraft:enchanted_golden_apple", "minecraft:nether_star", "minecraft:enchanted_book", "experience_bottle"] {
            assert!(Foil::Bedrock.always(both) && Foil::Java.always(both), "{both}");
        }
        assert!(Foil::Bedrock.always("minecraft:splash_potion") && !Foil::Java.always("minecraft:splash_potion"));
        assert!(Foil::Java.always("minecraft:written_book") && !Foil::Bedrock.always("minecraft:written_book"));
        assert!(!Foil::Bedrock.always("minecraft:golden_apple") && !Foil::Java.always("minecraft:diamond_sword"));
    }
}
