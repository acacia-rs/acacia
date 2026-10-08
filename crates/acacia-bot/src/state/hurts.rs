//! Hurt and death animations the server reports (`EntityEvent`), for clients that show them:
//! counts rather than times, so a watcher sees each new hurt by the count changing.

use std::collections::{HashMap, HashSet};

use acacia_client::proto::packets::{EntityEvent, EntityEventEventId};
use acacia_client::proto::{DecodeError, Packet, RawPacket};

#[derive(Debug, Default, Clone)]
pub struct Hurts {
    /// Hurt animations so far, by runtime id (this player's under its own id).
    hurts: HashMap<u64, u32>,
    /// Runtime ids whose death animation has played and who have not respawned.
    dying: HashSet<u64>,
}

impl Hurts {
    pub const PACKETS: &'static [u32] = &[EntityEvent::ID];

    pub fn apply(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        let p: EntityEvent = packet.decode()?;
        match p.event_id {
            EntityEventEventId::HurtAnimation => *self.hurts.entry(p.runtime_entity_id).or_default() += 1,
            EntityEventEventId::DeathAnimation => _ = self.dying.insert(p.runtime_entity_id),
            EntityEventEventId::Respawn => _ = self.dying.remove(&p.runtime_entity_id),
            _ => {}
        }
        Ok(())
    }

    /// How many times the entity with `runtime_id` has been hurt.
    pub fn count(&self, runtime_id: u64) -> u32 {
        self.hurts.get(&runtime_id).copied().unwrap_or(0)
    }

    pub fn dying(&self, runtime_id: u64) -> bool {
        self.dying.contains(&runtime_id)
    }
}
