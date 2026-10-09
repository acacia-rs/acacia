//! Status effect icons in the HUD, laid out as Java's `Gui.renderEffects`: from the top right
//! leftwards, 25 apart, beneficial effects in the first row and harmful ones in a second, each an
//! 18-pixel icon on a 24-pixel backing, blinking through its last 10 seconds.

use crate::draw::DrawList;
use crate::theme::Theme;

pub const BACKGROUND: &str = "hud/effect_background";
pub const BACKGROUND_AMBIENT: &str = "hud/effect_background_ambient";

/// Bedrock's effect id, Java's name for it (its icon is `textures/mob_effect/<name>.png`) and
/// whether Java puts it in the harmful row. Instant effects never show.
pub const EFFECTS: [(i32, &str, bool); 34] = [
    (1, "speed", false),
    (2, "slowness", true),
    (3, "haste", false),
    (4, "mining_fatigue", true),
    (5, "strength", false),
    (8, "jump_boost", false),
    (9, "nausea", true),
    (10, "regeneration", false),
    (11, "resistance", false),
    (12, "fire_resistance", false),
    (13, "water_breathing", false),
    (14, "invisibility", false),
    (15, "blindness", true),
    (16, "night_vision", false),
    (17, "hunger", true),
    (18, "weakness", true),
    (19, "poison", true),
    (20, "wither", true),
    (21, "health_boost", false),
    (22, "absorption", false),
    (23, "saturation", false),
    (24, "levitation", true),
    (25, "poison", true),
    (26, "conduit_power", false),
    (27, "slow_falling", false),
    (28, "bad_omen", true),
    (29, "hero_of_the_village", false),
    (30, "darkness", true),
    (31, "trial_omen", true),
    (32, "wind_charged", true),
    (33, "weaving", true),
    (34, "oozing", true),
    (35, "infested", true),
    (36, "raid_omen", true),
];

/// The atlas name of effect `name`'s icon.
pub fn sprite(name: &str) -> String {
    format!("effect/{name}")
}

/// Bedrock's icon file for Java's effect `name`, under `textures/ui`.
pub fn bedrock_file(name: &str) -> String {
    match name {
        "hero_of_the_village" => "village_hero_effect".to_owned(),
        _ => format!("{name}_effect"),
    }
}

/// One active effect: its id, the ticks it has left, and whether it is ambient (a beacon's).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Active {
    pub id: i32,
    pub ticks: i32,
    pub ambient: bool,
}

/// Where each effect's backing goes on a screen `width` GUI pixels wide, with its Java name.
fn layout(effects: &[Active], width: f32) -> Vec<(&'static str, Active, [f32; 2])> {
    let (mut good, mut bad) = (0.0, 0.0);
    let mut out = Vec::new();
    for active in effects {
        let Some(&(_, name, harmful)) = EFFECTS.iter().find(|(id, ..)| *id == active.id) else { continue };
        let column = if harmful { &mut bad } else { &mut good };
        *column += 1.0;
        out.push((name, *active, [width - 25.0 * *column, if harmful { 27.0 } else { 1.0 }]));
    }
    out
}

/// Java's blink for an effect with `ticks` left: steady above 200, then pulsing deeper to the end.
fn alpha(ticks: i32) -> f32 {
    if !(0..=200).contains(&ticks) {
        return 1.0;
    }
    let (d, left) = (ticks as f32, 10.0 - (ticks / 20) as f32);
    ((d / 10.0 / 5.0 * 0.5).clamp(0.0, 0.5) + (d * std::f32::consts::PI / 5.0).cos() * (left / 10.0 * 0.25).clamp(0.0, 0.25)).clamp(0.0, 1.0)
}

pub fn draw(list: &mut DrawList, theme: &Theme, effects: &[Active], width: f32) {
    for (name, active, [x, y]) in layout(effects, width) {
        let backing = if active.ambient { BACKGROUND_AMBIENT } else { BACKGROUND };
        if let Some(sprite) = theme.atlas.get(backing).or_else(|| theme.atlas.get(BACKGROUND)) {
            list.sprite(sprite, x, y, crate::draw::WHITE);
        }
        let alpha = if active.ambient { 1.0 } else { alpha(active.ticks) };
        if let Some(icon) = theme.atlas.get(&sprite(name)) {
            list.sprite(icon, x + 3.0, y + 3.0, [255, 255, 255, (alpha * 255.0) as u8]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn good_effects_fill_the_first_row_from_the_right_and_bad_ones_the_second() {
        let active = |id| Active { id, ticks: 600, ambient: false };
        let placed = layout(&[active(1), active(19), active(5), active(99)], 320.0);
        let at: Vec<_> = placed.iter().map(|(name, _, at)| (*name, *at)).collect();
        assert_eq!(at, [("speed", [295.0, 1.0]), ("poison", [295.0, 27.0]), ("strength", [270.0, 1.0])]);
        assert_eq!(bedrock_file("hero_of_the_village"), "village_hero_effect");
        // Steady, then a pulse: full at 200 ticks (cos 40π = 1), dark near the end.
        assert_eq!((alpha(600), alpha(-1)), (1.0, 1.0));
        assert!((alpha(200) - 0.5).abs() < 1e-3 && alpha(5) < 0.1, "{} {}", alpha(200), alpha(5));
    }
}
