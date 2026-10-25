//! `PlayerTabOverlay.extractTablistHearts` and its `HealthState`. Ported from Horizon's
//! `tab_list.rs` (MIT).

use crate::java::gui::{Gui, Metrics};
use crate::java::mth::{BLACK, clamp, floor};
use crate::java::text::{Lang, Text};

/// `PlayerTabOverlay.HealthState`: one player's hearts, blinking when they change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HealthState {
    last_value: i32,
    displayed_value: i32,
    last_update_tick: i64,
    blink_until_tick: i64,
}

impl HealthState {
    pub fn new(value: i32) -> HealthState {
        HealthState { last_value: value, displayed_value: value, last_update_tick: 0, blink_until_tick: 0 }
    }

    pub fn update(&mut self, value: i32, tick: i64) {
        if value != self.last_value {
            self.blink_until_tick = tick + if value < self.last_value { 20 } else { 10 };
            self.last_value = value;
            self.last_update_tick = tick;
        }
        if tick - self.last_update_tick > 20 {
            self.displayed_value = value;
        }
    }

    pub fn is_blinking(&self, tick: i64) -> bool {
        self.blink_until_tick > tick && (self.blink_until_tick - tick) % 6 >= 3
    }
}

/// `Float.toString`.
pub fn java_float(v: f32) -> String {
    if v == 0.0 || (1.0e-3..1.0e7).contains(&v.abs()) {
        return format!("{v:?}");
    }
    let s = format!("{v:E}");
    match s.split_once('E') {
        Some((m, e)) if !m.contains('.') => format!("{m}.0E{e}"),
        _ => s,
    }
}

/// `extractTablistHearts` between `left` and `right` on row `y`, `health` already updated.
#[allow(clippy::too_many_arguments)]
pub fn hearts(gui: &mut Gui, font: &dyn Metrics, lang: Lang, health: &HealthState, tick: i64, y: i32, left: i32, right: i32, score: i32) {
    let displayed = health.displayed_value;
    let full_hearts = -(-(score.max(displayed))).div_euclid(2);
    let to_render = score.max(displayed.max(20)) / 2;
    let blink = health.is_blinking(tick);
    if full_hearts <= 0 {
        return;
    }
    let per_heart = floor(((right - left - 4) as f32 / to_render as f32).min(9.0));
    if per_heart <= 3 {
        let pct = clamp(score as f32 / 20.0, 0.0, 1.0);
        let color = (((1.0 - pct) * 255.0) as i32) << 16 | ((pct * 255.0) as i32) << 8;
        let hearts = java_float(score as f32 / 2.0);
        let hp = Text::translate("multiplayer.player.list.hp", &[Text::literal(hearts.clone())], lang);
        let text = if right - font.width(&hp) >= left { hp } else { Text::literal(hearts) };
        let width = font.width(&text);
        gui.text(text, (right + left - width) / 2, y, color as u32 | BLACK, true);
        return;
    }
    let container = if blink { "hud/heart/container_blinking" } else { "hud/heart/container" };
    let x = |n: i32| left + n * per_heart;
    for n in full_hearts..to_render {
        gui.sprite(container, x(n), y, 9, 9);
    }
    for n in 0..full_hearts {
        gui.sprite(container, x(n), y, 9, 9);
        if blink {
            if n * 2 + 1 < displayed {
                gui.sprite("hud/heart/full_blinking", x(n), y, 9, 9);
            }
            if n * 2 + 1 == displayed {
                gui.sprite("hud/heart/half_blinking", x(n), y, 9, 9);
            }
        }
        if n * 2 + 1 < score {
            gui.sprite(if n >= 10 { "hud/heart/absorbing_full_blinking" } else { "hud/heart/full" }, x(n), y, 9, 9);
        }
        if n * 2 + 1 == score {
            gui.sprite(if n >= 10 { "hud/heart/absorbing_half_blinking" } else { "hud/heart/half" }, x(n), y, 9, 9);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_print_as_java_does() {
        assert_eq!(java_float(10.0), "10.0");
        assert_eq!(java_float(-1.5), "-1.5");
        assert_eq!(java_float(617_283.5), "617283.5");
        assert_eq!(java_float(1.0e7), "1.0E7");
        assert_eq!(java_float(0.0), "0.0");
    }

    #[test]
    fn blinks_after_a_change() {
        let mut h = HealthState::new(20);
        h.update(10, 100);
        assert!(!h.is_blinking(100));
        assert!(h.is_blinking(103));
        h.update(10, 121);
        assert_eq!(h.displayed_value, 10);
    }
}
