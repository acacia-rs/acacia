//! The Java theme from the jar's unpacked `assets/minecraft` (tools/lookbake unpacks it): one PNG
//! per sprite under `textures/gui/sprites`, named as the canonical names are.

use std::path::Path;

use super::{Style, Theme, java_font, png};
use crate::atlas::Atlas;
use crate::hud::sprite;

pub const STYLE: Style = Style { xp_colour: 0x80FF20, xp_outline: true };

const SPRITES: [&str; 15] = [
    sprite::HOTBAR, sprite::HOTBAR_SELECTION, sprite::CROSSHAIR, sprite::HEART_CONTAINER, sprite::HEART_FULL,
    sprite::HEART_HALF, sprite::FOOD_EMPTY, sprite::FOOD_FULL, sprite::FOOD_HALF, sprite::ARMOR_EMPTY,
    sprite::ARMOR_FULL, sprite::ARMOR_HALF, sprite::AIR, sprite::XP_BACKGROUND, sprite::XP_PROGRESS,
];

/// Loads what exists under `root`; missing sprites draw nothing.
pub fn load(root: &Path) -> Theme {
    let mut atlas = Atlas::new();
    for name in SPRITES {
        if let Some(image) = png(&root.join(format!("textures/gui/sprites/{name}.png"))) {
            atlas.add(name, &image);
        }
    }
    let font = java_font(root, &mut atlas);
    Theme { atlas, font, style: STYLE }
}
