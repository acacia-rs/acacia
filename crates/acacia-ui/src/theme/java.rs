//! The Java theme from the jar's unpacked `assets/minecraft` (tools/lookbake unpacks it): one PNG
//! per sprite under `textures/gui/sprites`, named as the canonical names are.

use std::path::Path;

use super::{Style, Theme, java_font, png};
use crate::atlas::Atlas;
use crate::effects;
use crate::hud::sprite;
use crate::overlay::BOSS_COLOURS;
use crate::widget::Widgets;
use crate::widget::java::Kit;

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
    for colour in BOSS_COLOURS {
        for part in ["background", "progress"] {
            let name = format!("boss_bar/{colour}_{part}");
            if let Some(image) = png(&root.join(format!("textures/gui/sprites/{name}.png"))) {
                atlas.add(&name, &image);
            }
        }
    }
    for name in [effects::BACKGROUND, effects::BACKGROUND_AMBIENT] {
        if let Some(image) = png(&root.join(format!("textures/gui/sprites/{name}.png"))) {
            atlas.add(name, &image);
        }
    }
    for (_, name, _) in effects::EFFECTS {
        if let Some(image) = png(&root.join(format!("textures/mob_effect/{name}.png"))) {
            atlas.add(&effects::sprite(name), &image);
        }
    }
    let font = java_font(root, &mut atlas);
    let widgets = Widgets::Java(Kit::load(root, &mut atlas));
    Theme { atlas, font, style: STYLE, widgets }
}
