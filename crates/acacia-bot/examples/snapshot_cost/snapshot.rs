//! Prototype `BotSnapshot`: owned, plain-data copies of what JS getters read.
//!
//! `GameState`, `PlayerState`, `Inventory` and `PlayerList` are not `Clone`, so nothing is cloned whole:
//! scalars are copied field by field, `ItemStack`s are summarised (the NBT tree and lore are left out;
//! `custom_name` is the one owned string), `Effects` and the player list are walked into new `Vec`s, and
//! item identifiers are either left as network ids (JS resolves them from a table sent once) or copied as
//! `String`s (`ItemRegistry::name` hands out `&str`, not its `Arc<str>`).

use acacia_bot::GameState;
use acacia_bot::proto::manual::Uuid;
use acacia_bot::state::Inventory;
use serde::{Serialize, Serializer};

/// Movement-owned values; they live on `Bot`, not `GameState`, so the example passes them in.
#[derive(Clone, Copy, Default)]
pub struct Motion {
    pub velocity: [f32; 3],
    pub on_ground: bool,
    pub gliding: bool,
}

/// Changes nearly every tick.
#[derive(Clone, Copy, Serialize)]
pub struct Hot {
    pub tick: u64,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub health: f32,
    pub max_health: f32,
    pub food: f32,
    pub saturation: f32,
    pub xp_level: i32,
    pub xp_progress: f32,
    pub air: i16,
    pub alive: bool,
    pub game_mode: i64,
    pub dimension: i32,
    pub held_slot: u8,
    pub flying: bool,
    pub sleeping: bool,
    pub riding: bool,
    pub gliding: bool,
}

#[derive(Clone, Serialize)]
pub struct Stack {
    pub window: i32,
    pub slot: u8,
    pub network_id: i32,
    pub count: u16,
    pub metadata: u32,
    pub stack_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_name: Option<String>,
}

#[derive(Clone, Copy, Serialize)]
pub struct EffectSummary {
    pub id: i32,
    pub amplifier: i32,
    pub duration: i32,
}

#[derive(Clone, Serialize)]
pub struct Player {
    #[serde(serialize_with = "as_str")]
    pub uuid: Uuid,
    pub username: String,
    pub xuid: String,
    /// i64 can pass 2^53: a decimal string, the design's `EntityId` rule.
    #[serde(serialize_with = "as_str")]
    pub entity_unique_id: i64,
}

/// Changes on inventory, effect or player-list events.
#[derive(Clone, Serialize)]
pub struct Cold {
    pub inventory: Vec<Stack>,
    pub cursor: Option<Stack>,
    pub effects: Vec<EffectSummary>,
    pub players: Vec<Player>,
    pub open_window: Option<i32>,
}

#[derive(Clone, Serialize)]
pub struct BotSnapshot {
    pub hot: Hot,
    pub cold: Cold,
}

fn as_str<T: std::fmt::Display, S: Serializer>(v: &T, s: S) -> Result<S::Ok, S::Error> {
    s.collect_str(v)
}

pub fn build_hot(state: &GameState, motion: &Motion, tick: u64) -> Hot {
    let p = &state.player;
    Hot {
        tick,
        position: [p.position.x, p.position.y, p.position.z],
        velocity: motion.velocity,
        yaw: p.yaw,
        pitch: p.pitch,
        on_ground: motion.on_ground,
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
        held_slot: state.inventory.selected_hotbar_slot,
        flying: p.abilities.flying,
        sleeping: p.is_sleeping(),
        riding: state.riding.is_riding(),
        gliding: motion.gliding,
    }
}

fn stack(state: &GameState, window: i32, slot: usize, s: &acacia_bot::state::ItemStack, names: bool) -> Stack {
    Stack {
        window,
        slot: slot as u8,
        network_id: s.network_id,
        count: s.count,
        metadata: s.metadata,
        stack_id: s.stack_network_id,
        name: if names { state.item_name(s).map(str::to_owned) } else { None },
        custom_name: s.custom_name.clone(),
    }
}

/// `names`: copy each stack's identifier as a `String` instead of leaving the network id alone.
pub fn build_cold(state: &GameState, names: bool) -> Cold {
    let inv = &state.inventory;
    let mut inventory = Vec::with_capacity(Inventory::MAIN_SLOTS + Inventory::ARMOR_SLOTS + 1);
    for (window, slot, s) in inv.iter().filter(|(w, _, _)| *w != Inventory::WINDOW_UI) {
        inventory.push(stack(state, window, slot, s, names));
    }
    let cursor = Some(inv.cursor()).filter(|s| !s.is_empty()).map(|s| stack(state, Inventory::WINDOW_UI, 0, s, names));
    let effects = state.player.effects.iter().map(|e| EffectSummary { id: e.id, amplifier: e.amplifier, duration: e.duration }).collect();
    let players = state
        .player_list
        .iter()
        .map(|p| Player { uuid: p.uuid, username: p.username.clone(), xuid: p.xuid.clone(), entity_unique_id: p.entity_unique_id })
        .collect();
    Cold { inventory, cursor, effects, players, open_window: state.containers.open.as_ref().map(|c| c.window_id) }
}

pub fn build_full(state: &GameState, motion: &Motion, tick: u64) -> BotSnapshot {
    BotSnapshot { hot: build_hot(state, motion, tick), cold: build_cold(state, false) }
}

fn opt_len(s: &Option<String>) -> usize {
    s.as_ref().map_or(0, String::capacity)
}

fn stack_heap(s: &Stack) -> usize {
    opt_len(&s.name) + opt_len(&s.custom_name)
}

/// Heap bytes owned by the snapshot (capacities, not allocator overhead).
pub fn cold_heap(c: &Cold) -> usize {
    c.inventory.capacity() * size_of::<Stack>()
        + c.inventory.iter().map(stack_heap).sum::<usize>()
        + c.cursor.as_ref().map_or(0, stack_heap)
        + c.effects.capacity() * size_of::<EffectSummary>()
        + c.players.capacity() * size_of::<Player>()
        + c.players.iter().map(|p| p.username.capacity() + p.xuid.capacity()).sum::<usize>()
}
