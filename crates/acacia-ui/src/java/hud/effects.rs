//! `Hud.extractEffects`: the status effect icons at the top right. Ported from Horizon's
//! `hud_overlays.rs` and `brewing.rs` (MIT).

use std::cmp::Ordering;

use super::sort::java_sort;
use crate::java::gui::Gui;
use crate::java::mth::{clamp, cos, white};

/// `MobEffects` in registration order: id, `getColor`, `isBeneficial`.
pub const EFFECTS: [(&str, i32, bool); 40] = [
    ("speed", 3402751, true),
    ("slowness", 9154528, false),
    ("haste", 14270531, true),
    ("mining_fatigue", 4866583, false),
    ("strength", 16762624, true),
    ("instant_health", 16262179, true),
    ("instant_damage", 11101546, false),
    ("jump_boost", 16646020, true),
    ("nausea", 5578058, false),
    ("regeneration", 13458603, true),
    ("resistance", 9520880, true),
    ("fire_resistance", 16750848, true),
    ("water_breathing", 10017472, true),
    ("invisibility", 16185078, true),
    ("blindness", 2039587, false),
    ("night_vision", 12779366, true),
    ("hunger", 5797459, false),
    ("weakness", 4738376, false),
    ("poison", 8889187, false),
    ("wither", 7561558, false),
    ("health_boost", 16284963, true),
    ("absorption", 2445989, true),
    ("saturation", 16262179, true),
    ("glowing", 9740385, false),
    ("levitation", 13565951, false),
    ("luck", 5882118, true),
    ("unluck", 12624973, false),
    ("slow_falling", 15978425, true),
    ("conduit_power", 1950417, true),
    ("dolphins_grace", 8954814, true),
    ("bad_omen", 745784, false),
    ("hero_of_the_village", 4521796, true),
    ("darkness", 2696993, false),
    ("trial_omen", 1484454, false),
    ("raid_omen", 14565464, false),
    ("wind_charged", 12438015, false),
    ("weaving", 7891290, false),
    ("oozing", 10092451, false),
    ("infested", 9214860, false),
    ("breath_of_the_nautilus", 65518, true),
];

/// A `MobEffectInstance` as `extractEffects` reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectIcon {
    /// The effect id without namespace; the sprite is `mob_effect/<id>`.
    pub id: String,
    pub beneficial: bool,
    pub color: i32,
    pub ambient: bool,
    /// Remaining ticks, -1 for infinite.
    pub duration: i32,
    pub show_icon: bool,
}

impl EffectIcon {
    /// An instance of the vanilla effect `id` (namespace optional); `None` for an unknown one.
    pub fn vanilla(id: &str, duration: i32, ambient: bool, show_icon: bool) -> Option<EffectIcon> {
        let id = id.trim_start_matches("minecraft:");
        let &(id, color, beneficial) = EFFECTS.iter().find(|e| e.0 == id)?;
        Some(EffectIcon { id: id.to_owned(), beneficial, color, ambient, duration, show_icon })
    }
}

/// `MobEffectInstance.compareTo`: not a total order, so the sort order matters.
pub fn compare_effects(a: &EffectIcon, o: &EffectIcon) -> Ordering {
    if (a.duration <= 32147 || o.duration <= 32147) && !(a.ambient && o.ambient) {
        a.ambient.cmp(&o.ambient).then((a.duration == -1).cmp(&(o.duration == -1))).then(a.duration.cmp(&o.duration)).then(a.color.cmp(&o.color))
    } else {
        a.ambient.cmp(&o.ambient).then(a.color.cmp(&o.color))
    }
}

/// The icon's alpha while it blinks out over its last 200 ticks.
fn blink_alpha(remaining: i32) -> f32 {
    let used = 10 - remaining / 20;
    let alpha = clamp(remaining as f32 / 10.0 / 5.0 * 0.5, 0.0, 0.5)
        + cos(f64::from(remaining as f32 * std::f32::consts::PI / 5.0)) * clamp(used as f32 / 10.0 * 0.25, 0.0, 0.25);
    clamp(alpha, 0.0, 1.0)
}

/// `Hud.extractEffects` for `effects` in `getActiveEffects` order; `demo` moves them below the
/// demo timer.
pub fn effects(gui: &mut Gui, demo: bool, effects: &[EffectIcon]) {
    let mut sorted: Vec<&EffectIcon> = effects.iter().collect();
    java_sort(&mut sorted, |a, b| compare_effects(b, a));
    let (mut beneficial, mut harmful) = (0, 0);
    for e in sorted.into_iter().filter(|e| e.show_icon) {
        let mut x = gui.width;
        let mut y = 1 + if demo { 15 } else { 0 };
        if e.beneficial {
            beneficial += 1;
            x -= 25 * beneficial;
        } else {
            harmful += 1;
            x -= 25 * harmful;
            y += 26;
        }
        let mut alpha = 1.0;
        if e.ambient {
            gui.sprite("hud/effect_background_ambient", x, y, 24, 24);
        } else {
            gui.sprite("hud/effect_background", x, y, 24, 24);
            if e.duration != -1 && e.duration <= 200 {
                alpha = blink_alpha(e.duration);
            }
        }
        gui.sprite_tinted(format!("mob_effect/{}", e.id), x + 3, y + 3, 18, 18, white(alpha));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java::gui::GuiDraw;

    #[test]
    fn beneficial_on_top_harmful_below() {
        let icons = [EffectIcon::vanilla("speed", 400, false, true).unwrap(), EffectIcon::vanilla("minecraft:poison", 400, false, true).unwrap()];
        let mut gui = Gui::new(100, 100);
        effects(&mut gui, false, &icons);
        let at: Vec<(i32, i32)> = gui.draws.iter().map(|d| match d {
            GuiDraw::Sprite { x, y, .. } => (*x, *y),
            _ => unreachable!(),
        }).collect();
        assert_eq!(at, [(75, 27), (78, 30), (75, 1), (78, 4)]);
    }

    #[test]
    fn blinks_out() {
        assert_eq!(blink_alpha(200), 0.5);
        assert!(blink_alpha(5) < 0.3);
    }
}
