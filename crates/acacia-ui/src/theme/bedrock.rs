//! The Bedrock theme from the resource pack: the PC HUD's `textures/ui` pieces, assembled into the
//! canonical sprites, and the classic `textures/gui/icons.png` sheet for what `textures/ui` lacks.
//! The pack has no font; the caller passes a Java asset root to borrow its (the same) one.

use std::path::Path;

use image::{RgbaImage, imageops};

use super::{Style, Theme, java_font, png};
use crate::atlas::Atlas;
use crate::hud::sprite;
use crate::overlay::BOSS_COLOURS;
use crate::widget::Widgets;
use crate::widget::bedrock::Kit;

pub const STYLE: Style = Style { xp_colour: 0x80FF00, xp_outline: false };

/// Single files under `textures/ui`.
const FILES: [(&str, &str); 11] = [
    (sprite::HOTBAR_SELECTION, "selected_hotbar_slot"),
    (sprite::HEART_CONTAINER, "heart_background"),
    (sprite::HEART_FULL, "heart"),
    (sprite::HEART_HALF, "heart_half"),
    (sprite::FOOD_EMPTY, "hunger_background"),
    (sprite::FOOD_FULL, "hunger_full"),
    (sprite::FOOD_HALF, "hunger_half"),
    (sprite::ARMOR_EMPTY, "armor_empty"),
    (sprite::ARMOR_FULL, "armor_full"),
    (sprite::ARMOR_HALF, "armor_half"),
    (sprite::AIR, "bubble"),
];

/// Rectangles of `textures/gui/icons.png`: x, y, width, height.
const ICONS: [(&str, [u32; 4]); 3] =
    [(sprite::CROSSHAIR, [0, 0, 15, 15]), (sprite::XP_BACKGROUND, [0, 64, 182, 5]), (sprite::XP_PROGRESS, [0, 69, 182, 5])];

/// The end caps are drawn at this opacity (`hud_screen.json`, `start_cap_image`).
const CAP_ALPHA: f32 = 0.65;

/// Loads what exists under the pack `root`; the font comes from `java_root` when given.
pub fn load(root: &Path, java_root: Option<&Path>) -> Theme {
    let mut atlas = Atlas::new();
    let ui = |name: &str| png(&root.join(format!("textures/ui/{name}.png")));
    if let Some(hotbar) = hotbar(&ui) {
        atlas.add(sprite::HOTBAR, &hotbar);
    }
    for (name, file) in FILES {
        if let Some(image) = ui(file) {
            atlas.add(name, &image);
        }
    }
    if let Some(icons) = png(&root.join("textures/gui/icons.png")) {
        for (name, [x, y, w, h]) in ICONS {
            atlas.add(name, &imageops::crop_imm(&icons, x, y, w, h).to_image());
        }
        // The sheet has one boss bar (the pink one); every colour draws it.
        let (bg, fill) = (imageops::crop_imm(&icons, 0, 74, 182, 5).to_image(), imageops::crop_imm(&icons, 0, 79, 182, 5).to_image());
        for colour in BOSS_COLOURS {
            atlas.add(&format!("boss_bar/{colour}_background"), &bg);
            atlas.add(&format!("boss_bar/{colour}_progress"), &fill);
        }
    }
    // 1937×333 does not fit the atlas; drawn about 150 GUI pixels wide.
    if let Some(logo) = ui("title") {
        let height = logo.height() * 512 / logo.width().max(1);
        atlas.add(crate::menu::LOGO, &imageops::resize(&logo, 512, height, imageops::FilterType::Triangle));
    }
    let font = java_root.and_then(|r| java_font(r, &mut atlas));
    let widgets = Widgets::Bedrock(Kit::load(root, &mut atlas));
    Theme { atlas, font, style: STYLE, widgets }
}

/// The 182×22 bar: start cap, nine 20×22 slots, end cap.
fn hotbar(ui: &dyn Fn(&str) -> Option<RgbaImage>) -> Option<RgbaImage> {
    let mut bar = RgbaImage::new(182, 22);
    let mut cap = |name: &str, x: i64| -> Option<()> {
        let mut image = ui(name)?;
        image.pixels_mut().for_each(|p| p.0[3] = (f32::from(p.0[3]) * CAP_ALPHA) as u8);
        imageops::replace(&mut bar, &image, x, 0);
        Some(())
    };
    cap("hotbar_start_cap", 0)?;
    cap("hotbar_end_cap", 181)?;
    for slot in 0..9 {
        imageops::replace(&mut bar, &ui(&format!("hotbar_{slot}"))?, 1 + 20 * slot, 0);
    }
    Some(bar)
}
