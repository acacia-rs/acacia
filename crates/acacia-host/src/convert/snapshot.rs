use acacia_api::{ColdSnapshot, Effect, HotSnapshot, PlayerInfo, Snapshot, Stack, UniqueId};
use acacia_bot::snapshot::{self as bot, BotSnapshot};
use acacia_bot::state::{ItemStack, PlayerListEntry};

pub(crate) fn snapshot(s: &BotSnapshot) -> Snapshot {
    Snapshot { hot: hot(&s.hot), cold: cold(&s.cold) }
}

pub(crate) fn hot(h: &bot::Hot) -> HotSnapshot {
    HotSnapshot {
        tick: h.tick,
        position: h.position.into(),
        velocity: h.velocity.into(),
        yaw: h.yaw,
        pitch: h.pitch,
        on_ground: h.on_ground,
        health: h.health,
        max_health: h.max_health,
        food: h.food,
        saturation: h.saturation,
        xp_level: h.xp_level,
        xp_progress: h.xp_progress,
        air: h.air,
        alive: h.alive,
        game_mode: h.game_mode as i32,
        dimension: h.dimension,
        held_slot: h.held_slot,
        open_window: h.open_window,
        flying: h.flying,
        sleeping: h.sleeping,
        riding: h.riding,
        gliding: h.gliding,
    }
}

fn cold(c: &bot::Cold) -> ColdSnapshot {
    let player = |p: &bot::Player| PlayerInfo {
        uuid: p.uuid.to_string(),
        username: p.username.to_string(),
        xuid: p.xuid.to_string(),
        unique_id: UniqueId(p.entity_unique_id),
    };
    ColdSnapshot {
        version: c.version,
        inventory: c.inventory.iter().map(stack).collect(),
        cursor: c.cursor.as_ref().map(stack),
        effects: c.effects.iter().map(|e| Effect { id: e.id, amplifier: e.amplifier, duration: e.duration }).collect(),
        players: c.players.iter().map(player).collect(),
    }
}

fn stack(s: &bot::Stack) -> Stack {
    Stack {
        window: s.window,
        slot: s.slot,
        network_id: s.network_id,
        count: s.count,
        metadata: s.metadata,
        stack_id: s.stack_id,
        custom_name: s.custom_name.clone(),
    }
}

/// The non-empty slots of window `window`.
pub(crate) fn slots(window: i32, items: &[ItemStack]) -> Vec<Stack> {
    let summary = |(slot, s): (usize, &ItemStack)| Stack {
        window,
        slot: slot as u8,
        network_id: s.network_id,
        count: s.count,
        metadata: s.metadata,
        stack_id: s.stack_network_id,
        custom_name: s.custom_name.clone(),
    };
    items.iter().enumerate().filter(|(_, s)| !s.is_empty()).map(summary).collect()
}

pub(crate) fn player(p: &PlayerListEntry) -> PlayerInfo {
    PlayerInfo { uuid: p.uuid.to_string(), username: p.username.to_string(), xuid: p.xuid.to_string(), unique_id: UniqueId(p.entity_unique_id) }
}
