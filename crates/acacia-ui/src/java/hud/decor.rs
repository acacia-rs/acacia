//! `GuiGraphicsExtractor.itemDecorations`: durability bar, cooldown shade and count. Ported from
//! Horizon's `hud_overlays.rs` (MIT).

use crate::java::gui::{Gui, Metrics};
use crate::java::mth::{BLACK, WHITE, ceil, clamp, floor, hsv_to_argb};

/// What `itemDecorations` reads from a non-empty stack.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemDecorations {
    pub count: i32,
    /// The text drawn instead of the count.
    pub count_text: Option<String>,
    /// `isBarVisible`: `getBarWidth` and `getBarColor` (RGB).
    pub bar: Option<(i32, u32)>,
    /// `ItemCooldowns.getCooldownPercent`.
    pub cooldown: f32,
}

/// `Item.getBarWidth` and `getBarColor` of a stack with `damage` out of `max_damage`
/// (0 when not damageable or unbreakable); `None` when undamaged.
pub fn damage_bar(damage: i32, max_damage: i32) -> Option<(i32, u32)> {
    if max_damage <= 0 || damage <= 0 {
        return None;
    }
    // Math.round(float): floor(x + 0.5).
    let width = floor(13.0 - damage as f32 * 13.0 / max_damage as f32 + 0.5).clamp(0, 13);
    let health = ((max_damage as f32 - damage as f32) / max_damage as f32).max(0.0);
    Some((width, hsv_to_argb(health / 3.0, 1.0, 1.0, 0)))
}

/// `ItemCooldowns.getCooldownPercent` for a cooldown `elapsed` whole ticks into `duration`; it is
/// gone once `elapsed` reaches `duration`.
pub fn cooldown_percent(duration: i32, elapsed: i32, partial: f32) -> f32 {
    if elapsed >= duration {
        return 0.0;
    }
    clamp((duration as f32 - (elapsed as f32 + partial)) / duration as f32, 0.0, 1.0)
}

/// `itemDecorations` over a stack drawn at (x, y).
pub fn item_decorations(gui: &mut Gui, font: &dyn Metrics, x: i32, y: i32, d: &ItemDecorations) {
    gui.push(Vec::new());
    if let Some((width, color)) = d.bar {
        let (left, top) = (x + 2, y + 13);
        gui.fill(left, top, left + 13, top + 2, BLACK);
        gui.fill(left, top, left + width, top + 1, color | BLACK);
    }
    if d.cooldown > 0.0 {
        let top = y + floor(16.0 * (1.0 - d.cooldown));
        let bottom = top + ceil(16.0 * d.cooldown);
        gui.fill(x, top, x + 16, bottom, 0x7FFF_FFFF);
    }
    if d.count != 1 || d.count_text.is_some() {
        let amount = d.count_text.clone().unwrap_or_else(|| d.count.to_string());
        let width = font.width(&amount.as_str().into());
        gui.text(amount, x + 19 - 2 - width, y + 6 + 3, WHITE, true);
    }
    gui.pop();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durability_bar() {
        assert_eq!(damage_bar(0, 100), None);
        assert_eq!(damage_bar(50, 100).map(|b| b.0), Some(7));
        assert_eq!(damage_bar(100, 100), Some((0, 0xFF0000)));
        assert_eq!(damage_bar(1, 1561), Some((13, 0x00FF00)));
    }

    #[test]
    fn cooldown_runs_out() {
        assert_eq!(cooldown_percent(20, 0, 0.0), 1.0);
        assert_eq!(cooldown_percent(20, 10, 0.5), 0.475);
        assert_eq!(cooldown_percent(20, 20, 0.0), 0.0);
    }
}
