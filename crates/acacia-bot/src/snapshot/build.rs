use std::sync::Arc;

use super::{BotSnapshot, Cold, EffectSummary, Hot, Player, Stack};
use crate::state::{Inventory, ItemStack};
use crate::{Bot, GameState};

impl Bot {
    /// A full snapshot built now, whether or not snapshots are published (see [`crate::snapshot`]).
    pub fn snapshot(&self) -> BotSnapshot {
        let tick = self.snapshots.as_ref().map_or(0, |p| p.tick);
        let entities = Arc::new(super::entity::build_list(&self.state.entities, 0, &Default::default(), &mut Default::default()));
        BotSnapshot { hot: self.hot_snapshot(tick), cold: Arc::new(build_cold(&self.state, 0)), entities }
    }

    pub(super) fn hot_snapshot(&self, tick: u64) -> Hot {
        let (s, p) = (&self.state, &self.state.player);
        let m = self.movement.as_ref();
        // A physics bot's body is its simulation's (the server's position only moves on corrections).
        let simulated = m.and_then(|m| Some((m.position()?, m.controls.yaw, m.controls.pitch)));
        let (position, yaw, pitch) = simulated.unwrap_or(([p.position.x, p.position.y, p.position.z], p.yaw, p.pitch));
        Hot {
            tick,
            position,
            velocity: m.and_then(|m| m.velocity()).unwrap_or_default(),
            yaw,
            pitch,
            on_ground: m.is_some_and(|m| m.on_ground()),
            health: p.health,
            max_health: p.max_health,
            food: p.hunger,
            saturation: p.saturation,
            xp_level: p.xp_level,
            xp_progress: p.xp_progress,
            air: p.air.0,
            alive: p.alive,
            game_mode: p.game_mode.to_raw(),
            dimension: p.dimension,
            held_slot: s.inventory.selected_hotbar_slot,
            open_window: s.containers.open.as_ref().map(|c| c.window_id),
            flying: p.abilities.flying,
            sleeping: p.is_sleeping(),
            riding: s.riding.is_riding(),
            gliding: m.is_some_and(|m| m.gliding()),
        }
    }
}

fn stack(window: i32, slot: usize, s: &ItemStack) -> Stack {
    Stack {
        window,
        slot: slot as u8,
        network_id: s.network_id,
        count: s.count,
        metadata: s.metadata,
        stack_id: s.stack_network_id,
        custom_name: s.custom_name.clone(),
    }
}

pub(super) fn build_cold(state: &GameState, version: u64) -> Cold {
    let inv = &state.inventory;
    let inventory = inv.iter().filter(|(w, _, s)| *w != Inventory::WINDOW_UI && !s.is_empty()).map(|(w, i, s)| stack(w, i, s)).collect();
    let cursor = Some(inv.cursor()).filter(|s| !s.is_empty()).map(|s| stack(Inventory::WINDOW_UI, 0, s));
    let effects = state.player.effects.iter().map(|e| EffectSummary { id: e.id, amplifier: e.amplifier, duration: e.duration }).collect();
    let players = state
        .player_list
        .iter()
        .map(|p| Player { uuid: p.uuid, username: p.username.clone(), xuid: p.xuid.clone(), entity_unique_id: p.entity_unique_id })
        .collect();
    Cold { version, inventory, cursor, effects, players }
}
