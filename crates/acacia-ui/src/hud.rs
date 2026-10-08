//! The in-game HUD, laid out as Java's `Gui` lays it out at GUI scale 1 (Bedrock's PC HUD shares the
//! layout): hotbar, crosshair, hearts, food, armour, air and experience. Sprites are looked up by the
//! names in [`sprite`]; a theme that lacks one draws nothing there.

use crate::atlas::{Atlas, Sprite};
use crate::draw::{DrawList, WHITE};
use crate::theme::Theme;

/// Canonical sprite names every theme loader fills.
pub mod sprite {
    pub const HOTBAR: &str = "hud/hotbar";
    pub const HOTBAR_SELECTION: &str = "hud/hotbar_selection";
    pub const CROSSHAIR: &str = "hud/crosshair";
    pub const HEART_CONTAINER: &str = "hud/heart/container";
    pub const HEART_FULL: &str = "hud/heart/full";
    pub const HEART_HALF: &str = "hud/heart/half";
    pub const FOOD_EMPTY: &str = "hud/food_empty";
    pub const FOOD_FULL: &str = "hud/food_full";
    pub const FOOD_HALF: &str = "hud/food_half";
    pub const ARMOR_EMPTY: &str = "hud/armor_empty";
    pub const ARMOR_FULL: &str = "hud/armor_full";
    pub const ARMOR_HALF: &str = "hud/armor_half";
    pub const AIR: &str = "hud/air";
    pub const XP_BACKGROUND: &str = "hud/experience_bar_background";
    pub const XP_PROGRESS: &str = "hud/experience_bar_progress";
}

/// What the HUD shows, from the player's state.
#[derive(Debug, Clone, PartialEq)]
pub struct HudState {
    pub health: f32,
    pub max_health: f32,
    pub food: f32,
    /// 0 to 20.
    pub armor: u32,
    /// Ticks of breath left; `None` when not under water.
    pub air: Option<(u32, u32)>,
    pub xp_level: u32,
    /// 0 to 1.
    pub xp_progress: f32,
    pub selected: u8,
    /// Hotbar slots: the item's icon in the atlas and the stack size.
    pub hotbar: [Option<(Sprite, u16)>; 9],
    /// Creative and spectator show no health, food or experience.
    pub survival: bool,
}

/// Lays the HUD out for a window `size` GUI pixels big.
pub fn draw(list: &mut DrawList, theme: &Theme, state: &HudState, size: [f32; 2]) {
    let [w, h] = size;
    let atlas = &theme.atlas;
    let sprite = |name| atlas.get(name);
    let centre = (w / 2.0).floor();
    if let Some(cross) = sprite(sprite::CROSSHAIR) {
        list.sprite(cross, ((w - cross.width as f32) / 2.0).floor(), ((h - cross.height as f32) / 2.0).floor(), WHITE);
    }
    let left = centre - 91.0;
    if let Some(bar) = sprite(sprite::HOTBAR) {
        list.sprite(bar, left, h - 22.0, WHITE);
    }
    if let Some(sel) = sprite(sprite::HOTBAR_SELECTION) {
        list.sprite(sel, left - 1.0 + f32::from(state.selected) * 20.0, h - 23.0, WHITE);
    }
    for (slot, item) in state.hotbar.iter().enumerate() {
        let Some((icon, count)) = item else { continue };
        let (x, y) = (left + 3.0 + slot as f32 * 20.0, h - 19.0);
        list.sprite_stretched(*icon, [x, y, x + 16.0, y + 16.0], WHITE);
        if let (true, Some(font)) = (*count > 1, &theme.font) {
            let text = count.to_string();
            font.draw(list, &text, x + 17.0 - font.width(&text), y + 9.0, 0xFFFFFF, 1.0, true);
        }
    }
    if !state.survival {
        return;
    }
    xp(list, theme, state, centre, h);
    let top = h - 39.0;
    let hearts_rows = (state.max_health / 2.0 / 10.0).ceil().max(1.0);
    let row_gap = (10.0 - (hearts_rows - 2.0)).max(3.0);
    icons(list, atlas, (state.max_health / 2.0).ceil() as u32, state.health, |i| (left + (i % 10) as f32 * 8.0, top - (i / 10) as f32 * row_gap), [sprite::HEART_CONTAINER, sprite::HEART_FULL, sprite::HEART_HALF]);
    icons(list, atlas, 10, state.food, |i| (centre + 91.0 - 9.0 - i as f32 * 8.0, top), [sprite::FOOD_EMPTY, sprite::FOOD_FULL, sprite::FOOD_HALF]);
    let above_hearts = top - (hearts_rows - 1.0) * row_gap - 10.0;
    if state.armor > 0 {
        icons(list, atlas, 10, state.armor as f32, |i| (left + i as f32 * 8.0, above_hearts), [sprite::ARMOR_EMPTY, sprite::ARMOR_FULL, sprite::ARMOR_HALF]);
    }
    if let (Some((air, max)), Some(bubble)) = (state.air, sprite(sprite::AIR)) {
        let full = (air * 10).div_ceil(max.max(1));
        for i in 0..full.min(10) {
            list.sprite(bubble, centre + 91.0 - 9.0 - i as f32 * 8.0, top - 10.0, WHITE);
        }
    }
}

/// `count` icons of a row worth 2 points each: the empty sprite, then full or half over it.
fn icons(list: &mut DrawList, atlas: &Atlas, count: u32, value: f32, at: impl Fn(u32) -> (f32, f32), [empty, full, half]: [&str; 3]) {
    let points = value.ceil().max(0.0) as u32;
    for i in 0..count {
        let (x, y) = at(i);
        if let Some(s) = atlas.get(empty) {
            list.sprite(s, x, y, WHITE);
        }
        let over = if points >= i * 2 + 2 { full } else if points == i * 2 + 1 { half } else { continue };
        if let Some(s) = atlas.get(over) {
            list.sprite(s, x, y, WHITE);
        }
    }
}

fn xp(list: &mut DrawList, theme: &Theme, state: &HudState, centre: f32, h: f32) {
    let (atlas, style) = (&theme.atlas, theme.style);
    let left = centre - 91.0;
    if let Some(bg) = atlas.get(sprite::XP_BACKGROUND) {
        list.sprite(bg, left, h - 29.0, WHITE);
    }
    if let Some(fill) = atlas.get(sprite::XP_PROGRESS) {
        let filled = (state.xp_progress.clamp(0.0, 1.0) * 183.0).floor().min(fill.width as f32);
        if filled > 0.0 {
            list.sprite_part(fill, left, h - 29.0, [0.0, 0.0, filled, fill.height as f32], WHITE);
        }
    }
    if let (true, Some(font)) = (state.xp_level > 0, &theme.font) {
        let text = state.xp_level.to_string();
        let x = centre - (font.width(&text) / 2.0).floor();
        let y = h - 35.0;
        if style.xp_outline {
            for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                font.draw(list, &text, x + dx, y + dy, 0x000000, 1.0, false);
            }
        }
        font.draw(list, &text, x, y, style.xp_colour, 1.0, !style.xp_outline);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    fn atlas() -> Atlas {
        let mut a = Atlas::new();
        for (name, w, h) in [(sprite::HOTBAR, 182, 22), (sprite::HOTBAR_SELECTION, 24, 23), (sprite::HEART_CONTAINER, 9, 9), (sprite::HEART_FULL, 9, 9), (sprite::HEART_HALF, 9, 9)] {
            a.add(name, &RgbaImage::new(w, h));
        }
        a
    }

    fn state() -> HudState {
        HudState { health: 15.0, max_health: 20.0, food: 20.0, armor: 0, air: None, xp_level: 0, xp_progress: 0.0, selected: 2, hotbar: [None; 9], survival: true }
    }

    #[test]
    fn hotbar_and_hearts_sit_where_java_puts_them() {
        let theme = Theme { atlas: atlas(), font: None, style: crate::theme::java::STYLE };
        let atlas = &theme.atlas;
        let mut list = DrawList::new(1.0);
        draw(&mut list, &theme, &state(), [320.0, 240.0]);
        let at = |name| list.quads.iter().filter(|q| q.uv[0] == atlas.get(name).unwrap().x as f32 && q.uv[1] == atlas.get(name).unwrap().y as f32).map(|q| [q.rect[0], q.rect[1]]).collect::<Vec<_>>();
        assert_eq!(at(sprite::HOTBAR), [[69.0, 218.0]]);
        assert_eq!(at(sprite::HOTBAR_SELECTION), [[108.0, 217.0]]);
        let full = at(sprite::HEART_FULL);
        assert_eq!(full.len(), 7, "15 health: seven full hearts and a half");
        assert_eq!(full[0], [69.0, 201.0]);
        assert_eq!(at(sprite::HEART_HALF), [[125.0, 201.0]]);
    }
}
