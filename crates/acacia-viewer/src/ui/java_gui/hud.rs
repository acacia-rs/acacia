//! `Hud.extractRenderState` for the player the viewer shows.

use std::time::Instant;

use acacia_bot::proto::types::GameMode;
use acacia_ui::java::gui::Gui;
use acacia_ui::java::hud::health::{Health, HeartKind};
use acacia_ui::java::hud::hotbar::{self, Hotbar, OFFHAND_KEY};

use super::JavaGui;
use crate::control::Stack;
use crate::me::Me;

// Bedrock's effect ids.
const REGENERATION: i32 = 10;
const HUNGER: i32 = 17;
const POISON: i32 = 19;
const WITHER: i32 = 20;

impl JavaGui {
    /// The crosshair, hotbar, health, food, air and experience for `me`.
    pub fn hud(&mut self, me: &Me, size: [i32; 2], crosshair: bool, now: Instant) -> Gui {
        let mut gui = Gui::new(size[0], size[1]);
        let spectator = matches!(me.game_mode, GameMode::Spectator | GameMode::SurvivalSpectator | GameMode::CreativeSpectator);
        let survival = matches!(me.game_mode, GameMode::Survival | GameMode::Adventure);
        if crosshair && !spectator {
            hotbar::crosshair(&mut gui, None, false);
        }
        if !spectator {
            let bar = Hotbar {
                selected: i32::from(me.hotbar % 9),
                right_handed: true,
                slots: std::array::from_fn(|i| me.items[i].as_ref().map(|_| 0.0)),
                offhand: me.offhand.as_ref().map(|_| 0.0),
                attack: None,
            };
            hotbar::hotbar(&mut gui, &bar);
        }
        if survival {
            let millis = self.millis(now);
            let has = |id| me.effects.iter().any(|e| e.0 == id);
            let kind = if has(POISON) {
                HeartKind::Poisoned
            } else if has(WITHER) {
                HeartKind::Withered
            } else {
                HeartKind::Normal
            };
            let (air, max_air) = me.air.map_or((300, 300), |(a, m)| (a as i32, m as i32));
            let state = Health {
                tick: (millis / 50) as i32,
                millis,
                health: me.health,
                max_health: me.max_health,
                absorption: 0.0,
                armor: me.armor as i32,
                food: me.food as i32,
                saturation: 0.0,
                kind,
                regeneration: has(REGENERATION),
                hunger: has(HUNGER),
                hardcore: false,
                damage_cooldown: false,
                air,
                max_air,
                under_water: me.air.is_some(),
                vehicle_hearts: 0,
            };
            self.health.extract(&mut gui, &state);
        }
        hotbar::experience(&mut gui, &self.fonts, me.xp_progress, if survival { me.xp_level } else { 0 }, survival);
        gui
    }
}

/// The hotbar's item keys: slot numbers, and `OFFHAND_KEY`.
pub fn hotbar_stack(me: &Me, key: i32) -> Option<Stack> {
    match key {
        OFFHAND_KEY => me.offhand.clone(),
        k => me.items.get(usize::try_from(k).ok()?)?.clone(),
    }
}
