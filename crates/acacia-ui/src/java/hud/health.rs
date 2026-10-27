//! `Hud.extractPlayerHealth`: armour, hearts of every kind, food and air bubbles.

use crate::java::gui::Gui;
use crate::java::mth::{JavaRandom, ceil};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum HeartKind {
    #[default]
    Normal,
    Poisoned,
    Withered,
    Frozen,
}

/// What `Hud` keeps between frames (`lastHealth`, `displayHealth`, `lastHealthTime`,
/// `healthBlinkTime`).
#[derive(Clone, Debug, Default)]
pub struct HealthHud {
    pub last_health: i32,
    pub display_health: i32,
    /// `Util.getMillis()` of the last change.
    pub last_health_time: i64,
    pub health_blink_time: i64,
}

#[derive(Clone, Debug, Default)]
pub struct Health {
    /// `Hud.tickCount`.
    pub tick: i32,
    /// `Util.getMillis()` now.
    pub millis: i64,
    pub health: f32,
    pub max_health: f32,
    pub absorption: f32,
    pub armor: i32,
    pub food: i32,
    pub saturation: f32,
    pub kind: HeartKind,
    pub regeneration: bool,
    pub hunger: bool,
    pub hardcore: bool,
    /// `damageCooldownTime > 0`.
    pub damage_cooldown: bool,
    pub air: i32,
    pub max_air: i32,
    /// `isEyeInFluid(WATER)`.
    pub under_water: bool,
    /// `getVehicleMaxHearts` of a ridden living vehicle; 0 for none.
    pub vehicle_hearts: i32,
}

/// `Hud.HeartType`'s sprite: `kind` `None` is the container.
fn heart(kind: Option<HeartKind>, absorbing: bool, hardcore: bool, half: bool, blink: bool) -> String {
    let base = match (kind, absorbing) {
        (None, _) => {
            let hc = if hardcore { "_hardcore" } else { "" };
            let bl = if blink { "_blinking" } else { "" };
            return format!("hud/heart/container{hc}{bl}");
        }
        (_, true) => "absorbing_",
        (Some(HeartKind::Normal), _) => "",
        (Some(HeartKind::Poisoned), _) => "poisoned_",
        (Some(HeartKind::Withered), _) => "withered_",
        (Some(HeartKind::Frozen), _) => "frozen_",
    };
    let hc = if hardcore { "hardcore_" } else { "" };
    let part = if half { "half" } else { "full" };
    let bl = if blink { "_blinking" } else { "" };
    format!("hud/heart/{base}{hc}{part}{bl}")
}

impl HealthHud {
    /// Updates the kept state as the game does while drawing, and draws.
    pub fn extract(&mut self, gui: &mut Gui, i: &Health) {
        let tick = i64::from(i.tick);
        let current = ceil(i.health);
        let blink = self.health_blink_time > tick && (self.health_blink_time - tick) / 3 % 2 == 1;
        if current < self.last_health && i.damage_cooldown {
            self.last_health_time = i.millis;
            self.health_blink_time = tick + 20;
        } else if current > self.last_health && i.damage_cooldown {
            self.last_health_time = i.millis;
            self.health_blink_time = tick + 10;
        }
        if i.millis - self.last_health_time > 1000 {
            self.display_health = current;
            self.last_health_time = i.millis;
        }
        self.last_health = current;
        let old = self.display_health;
        let mut random = JavaRandom::new(i64::from(i.tick.wrapping_mul(312_871)));
        let (x_left, x_right, y_base) = (gui.width / 2 - 91, gui.width / 2 + 91, gui.height - 39);
        let max_health = i.max_health.max(old.max(current) as f32);
        let absorption = ceil(i.absorption);
        let rows = ceil((max_health + absorption as f32) / 2.0 / 10.0);
        let row_height = (10 - (rows - 2)).max(3);
        let mut y_air = y_base - 10;
        let offset_index = if i.regeneration { i.tick % ceil(max_health + 5.0) } else { -1 };
        if i.armor > 0 {
            let y = y_base - (rows - 1) * row_height - 10;
            for n in 0..10 {
                let sprite = match (n * 2 + 1).cmp(&i.armor) {
                    std::cmp::Ordering::Less => "hud/armor_full",
                    std::cmp::Ordering::Equal => "hud/armor_half",
                    std::cmp::Ordering::Greater => "hud/armor_empty",
                };
                gui.sprite(sprite, x_left + n * 8, y, 9, 9);
            }
        }
        let containers = f64::from(max_health / 2.0).ceil() as i32;
        let absorb_containers = (f64::from(absorption) / 2.0).ceil() as i32;
        for c in (0..containers + absorb_containers).rev() {
            let (row, column) = (c / 10, c % 10);
            let x = x_left + column * 8;
            let mut y = y_base - row * row_height;
            if current + absorption <= 4 {
                y += random.next_int_bound(2);
            }
            if c < containers && c == offset_index {
                y -= 2;
            }
            gui.sprite(heart(None, false, i.hardcore, false, blink), x, y, 9, 9);
            let halves = c * 2;
            if c >= containers {
                let ah = halves - containers * 2;
                if ah < absorption {
                    let withered = i.kind == HeartKind::Withered;
                    gui.sprite(heart(Some(i.kind), !withered, i.hardcore, ah + 1 == absorption, false), x, y, 9, 9);
                }
            }
            if blink && halves < old {
                gui.sprite(heart(Some(i.kind), false, i.hardcore, halves + 1 == old, true), x, y, 9, 9);
            }
            if halves < current {
                gui.sprite(heart(Some(i.kind), false, i.hardcore, halves + 1 == current, false), x, y, 9, 9);
            }
        }
        if i.vehicle_hearts == 0 {
            let suffix = if i.hunger { "_hunger" } else { "" };
            for n in 0..10 {
                let mut y = y_base;
                if i.saturation <= 0.0 && i.tick % (i.food * 3 + 1) == 0 {
                    y += random.next_int_bound(3) - 1;
                }
                let x = x_right - n * 8 - 9;
                gui.sprite(format!("hud/food_empty{suffix}"), x, y, 9, 9);
                if n * 2 + 1 < i.food {
                    gui.sprite(format!("hud/food_full{suffix}"), x, y, 9, 9);
                }
                if n * 2 + 1 == i.food {
                    gui.sprite(format!("hud/food_half{suffix}"), x, y, 9, 9);
                }
            }
            y_air -= 10;
        }
        air(gui, i, x_right, y_air, &mut random);
    }
}

/// `Hud.extractAirBubbles`.
fn air(gui: &mut Gui, i: &Health, x_right: i32, y_air: i32, random: &mut JavaRandom) {
    let max_air = i.max_air.max(1);
    let air = i.air.clamp(0, max_air);
    if !i.under_water && air >= max_air {
        return;
    }
    let rows = (f64::from(i.vehicle_hearts) / 10.0).ceil() as i32;
    let y = y_air - (rows - 1) * 10;
    let bubble = |offset: i32| ceil(((air + offset) * 10) as f32 / max_air as f32);
    let (full, popping) = (bubble(-2), bubble(0));
    let empty = 10 - bubble(i32::from(air != 0 && i.under_water));
    for b in 1..=10 {
        let x = x_right - (b - 1) * 8 - 9;
        if b <= full {
            gui.sprite("hud/air", x, y, 9, 9);
        } else if full != popping && b == popping && i.under_water {
            gui.sprite("hud/air_bursting", x, y, 9, 9);
        } else if b > 10 - empty {
            let wobble = if empty == 10 && i.tick % 2 == 0 { random.next_int_bound(2) } else { 0 };
            gui.sprite("hud/air_empty", x, y + wobble, 9, 9);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heart_sprites_follow_heart_type() {
        assert_eq!(heart(None, false, true, false, true), "hud/heart/container_hardcore_blinking");
        assert_eq!(heart(Some(HeartKind::Normal), true, true, true, false), "hud/heart/absorbing_hardcore_half");
        assert_eq!(heart(Some(HeartKind::Frozen), false, false, false, true), "hud/heart/frozen_full_blinking");
        assert_eq!(heart(Some(HeartKind::Normal), false, false, true, false), "hud/heart/half");
    }

    #[test]
    fn fifteen_health_is_seven_hearts_and_a_half() {
        let mut gui = Gui::new(320, 240);
        let state = Health { health: 15.0, max_health: 20.0, food: 20, saturation: 5.0, air: 300, max_air: 300, millis: 5000, ..Health::default() };
        HealthHud::default().extract(&mut gui, &state);
        let names: Vec<_> = gui.draws.iter().filter_map(|d| match d {
            crate::java::gui::GuiDraw::Sprite { sprite, x, y, .. } if sprite.starts_with("hud/heart/") && !sprite.contains("container") => Some((sprite.to_string(), *x, *y)),
            _ => None,
        }).collect();
        assert_eq!(names.len(), 8);
        assert_eq!(names[0], ("hud/heart/half".to_owned(), 125, 201), "drawn from the last container down");
    }
}
