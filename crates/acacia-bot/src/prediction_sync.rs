//! `ClientMovementPredictionSync`, which vanilla sends 9.9 s after the spawn `PlayerAction(Respawn)`
//! (both 2026-10-01 captures) and later when its state changes. Its exact later triggers are unknown;
//! health or hunger changes are the ones the capture shows, so those are what we follow.

use acacia_client::proto::packets::{
    ClientMovementPredictionSync, ClientMovementPredictionSyncBoundingBox, SetEntityData, UpdateAttributes,
};
use acacia_client::proto::types::MetadataDictionaryItemValue;
use acacia_client::proto::{Packet, RawPacket};

use crate::state::PlayerState;

/// ~9.9 s at the ~51 ms tick cadence.
const FIRST_DELAY_TICKS: u32 = 194;
const MIN_GAP_TICKS: u32 = 200;
/// The first sync reports 0.7 although the server says 0.1 (both captures); later ones use the server's value.
const FIRST_MOVEMENT_SPEED: f32 = 0.7;

pub(crate) struct PredictionSync {
    flags: i64,
    flags_extended: i64,
    movement: f32,
    underwater_movement: f32,
    lava_movement: f32,
    friction: f32,
    bounciness: f32,
    air_drag: f32,
    due_in: Option<u32>,
    since_last: u32,
    last: Option<(f32, f32)>,
}

impl Default for PredictionSync {
    fn default() -> Self {
        Self {
            flags: 0,
            flags_extended: 0,
            movement: 0.1,
            underwater_movement: 0.02,
            lava_movement: 0.02,
            friction: 1.0,
            bounciness: 0.0,
            air_drag: 1.0,
            due_in: None,
            since_last: 0,
            last: None,
        }
    }
}

impl PredictionSync {
    pub const PACKETS: &[u32] = &[SetEntityData::ID, UpdateAttributes::ID];

    pub fn apply(&mut self, packet: &RawPacket, runtime_id: u64) {
        if let Ok(p) = packet.decode::<SetEntityData>()
            && p.runtime_entity_id == runtime_id
        {
            for item in p.metadata {
                match item.value {
                    MetadataDictionaryItemValue::Flags(f) => self.flags = f.0,
                    MetadataDictionaryItemValue::FlagsExtended(f) => self.flags_extended = f.0,
                    _ => {}
                }
            }
        } else if let Ok(p) = packet.decode::<UpdateAttributes>()
            && p.runtime_entity_id == runtime_id
        {
            for a in p.attributes {
                let field = match a.name.as_str() {
                    "minecraft:movement" => &mut self.movement,
                    "minecraft:underwater_movement" => &mut self.underwater_movement,
                    "minecraft:lava_movement" => &mut self.lava_movement,
                    "minecraft:friction_modifier" => &mut self.friction,
                    "minecraft:bounciness" => &mut self.bounciness,
                    "minecraft:air_drag_modifier" => &mut self.air_drag,
                    _ => continue,
                };
                *field = a.current;
            }
        }
    }

    /// The player finished spawning (vanilla's `PlayerAction(Respawn)`); the first sync follows.
    pub fn spawned(&mut self) {
        if self.last.is_none() {
            self.due_in = Some(FIRST_DELAY_TICKS);
        }
    }

    pub fn tick(&mut self, player: &PlayerState) -> Option<ClientMovementPredictionSync> {
        self.since_last = self.since_last.saturating_add(1);
        if !player.alive {
            return None;
        }
        let first = match &mut self.due_in {
            Some(0) => true,
            Some(n) => {
                *n -= 1;
                return None;
            }
            None => false,
        };
        let state = (player.health, player.hunger);
        let changed = self.last.is_some_and(|last| last != state) && self.since_last >= MIN_GAP_TICKS;
        if !first && !changed {
            return None;
        }
        self.due_in = None;
        self.since_last = 0;
        self.last = Some(state);
        Some(self.packet(player, first))
    }

    fn packet(&self, player: &PlayerState, first: bool) -> ClientMovementPredictionSync {
        let zigzag = ((player.unique_entity_id << 1) ^ (player.unique_entity_id >> 63)) as u64;
        ClientMovementPredictionSync {
            data_flags: (u128::from(self.flags_extended as u64) << 64) | u128::from(self.flags as u64),
            bounding_box: ClientMovementPredictionSyncBoundingBox { scale: 1.0, width: 0.6, height: 1.8 },
            movement_speed: if first { FIRST_MOVEMENT_SPEED } else { self.movement },
            underwater_movement_speed: self.underwater_movement,
            lava_movement_speed: self.lava_movement,
            jump_strength: 0.0,
            health: player.health,
            hunger: player.hunger,
            unknown_attribute_1: self.friction,
            unknown_attribute_2: self.bounciness,
            unknown_attribute_3: self.air_drag,
            entity_runtime_id: zigzag,
            is_flying: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player() -> PlayerState {
        PlayerState { unique_entity_id: -51_539_607_551, alive: true, health: 20.0, hunger: 20.0, ..PlayerState::default() }
    }

    #[test]
    fn first_sync_after_spawn_like_vanilla() {
        let mut sync = PredictionSync { flags: 0x3_0008_0008_0000, flags_extended: 0x2000_0200_0000, ..Default::default() };
        let p = player();
        assert!(sync.tick(&p).is_none(), "nothing before spawn completes");
        sync.spawned();
        let sent: Vec<_> = (0..400).filter_map(|i| sync.tick(&p).map(|s| (i, s))).collect();
        assert_eq!(sent.len(), 1);
        let (at, s) = &sent[0];
        assert_eq!(*at, FIRST_DELAY_TICKS);
        let mut wire = bytes::BytesMut::new();
        s.write(&mut wire);
        // Vanilla's bytes for the same state (capture session 2).
        let vanilla = "8080a0808081c08180808080a08080100000803f9a99193f6666e63f3333333f0ad7a33c0ad7a33c000000000000a0410000a0410000803f000000000000803ffdffffffff0200";
        assert_eq!(wire.iter().map(|b| format!("{b:02x}")).collect::<String>(), vanilla);
    }

    #[test]
    fn resends_on_health_or_hunger_change_at_most_every_ten_seconds() {
        let mut sync = PredictionSync::default();
        let mut p = player();
        sync.spawned();
        (0..=FIRST_DELAY_TICKS).for_each(|_| _ = sync.tick(&p));
        p.hunger = 17.0;
        let at = (1..=MIN_GAP_TICKS).find(|_| sync.tick(&p).is_some());
        assert_eq!(at, Some(MIN_GAP_TICKS));
        assert!((0..500).all(|_| sync.tick(&p).is_none()), "unchanged state sends nothing");
    }
}
