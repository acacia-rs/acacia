use acacia_client::proto::codec::read_varint64;
use acacia_client::proto::packets::{
    AddEntity, AddItemEntity, ChangeDimension, EntityEvent, EntityEventEventId, RemoveEntity, SetEntityMotion, TakeItemEntity,
};
use acacia_client::proto::types::{MetadataDictionaryItemKey, MetadataDictionaryItemValue, MetadataDictionaryItemValueDefault, Vec3f};
use acacia_client::proto::{DecodeError, Packet, RawPacket};

use super::{ItemStack, Me};

pub const FISHING_HOOK_KIND: &str = "minecraft:fishing_hook";

/// A hook resting on the water moves vertically slower than this (blocks/tick)...
const RESTING_SPEED: f32 = 0.1;
/// ...and a bite pulls it down faster than this in one update (Java's bobber dip is 0.24-0.4).
const DIP_SPEED: f32 = 0.2;
/// ...after this many resting updates in a row: the apex of the cast (vy near 0, then falling) is no bite.
const SETTLED_UPDATES: u32 = 3;
/// Fishing loot in flight kept at most (each lives a second or two).
const MAX_LOOT: usize = 16;

/// The local player's cast hook.
#[derive(Debug, Clone, PartialEq)]
pub struct FishingHook {
    pub runtime_id: u64,
    pub unique_id: i64,
    pub spawned_at: Vec3f,
    /// Bites seen, each catchable for a moment: `FishHookHook` events (BDS and server software) or a
    /// sudden dip of the resting hook (Geyser sends no hook events).
    pub bites: u32,
    resting_updates: u32,
}

/// Only the own fishing hook, so fishing works without `Trackers::entities`.
#[derive(Debug, Default)]
pub struct Fishing {
    pub hook: Option<FishingHook>,
    /// The last fishing loot this player picked up. BDS announces the pickup (`TakeItemEntity`)
    /// but sends no slot update for it, so the inventory never shows the catch.
    pub catch: Option<ItemStack>,
    /// Fishing loot in flight (`AddItemEntity` from fishing): runtime id, unique id, stack.
    loot: Vec<(u64, i64, ItemStack)>,
}

impl Fishing {
    pub const PACKETS: &'static [u32] = &[
        AddEntity::ID,
        RemoveEntity::ID,
        EntityEvent::ID,
        SetEntityMotion::ID,
        ChangeDimension::ID,
        AddItemEntity::ID,
        TakeItemEntity::ID,
    ];

    pub fn apply(&mut self, packet: &RawPacket, me: &Me) -> Result<(), DecodeError> {
        match packet.id {
            AddItemEntity::ID => {
                let p: AddItemEntity = packet.decode()?;
                if p.is_from_fishing {
                    if self.loot.len() == MAX_LOOT {
                        self.loot.remove(0);
                    }
                    self.loot.push((p.runtime_entity_id, p.entity_id_self, p.item.into()));
                }
            }
            TakeItemEntity::ID => {
                let p: TakeItemEntity = packet.decode()?;
                if let Some(i) = self.loot.iter().position(|(id, ..)| *id == p.runtime_entity_id) {
                    let (_, _, stack) = self.loot.remove(i);
                    if u64::from(p.target) == me.runtime_entity_id {
                        self.catch = Some(stack);
                    }
                }
            }
            AddEntity::ID => {
                let p: AddEntity = packet.decode()?;
                // Geyser fills the owner with the unique id, Dragonfly with the runtime id.
                let owner = owner(&p);
                if p.entity_type == FISHING_HOOK_KIND
                    && (owner == Some(me.unique_entity_id) || owner == Some(me.runtime_entity_id as i64))
                {
                    self.hook = Some(FishingHook {
                        runtime_id: p.runtime_id,
                        unique_id: p.unique_id,
                        spawned_at: p.position,
                        bites: 0,
                        resting_updates: 0,
                    });
                }
            }
            RemoveEntity::ID => {
                let unique = packet.decode::<RemoveEntity>()?.entity_id_self;
                if self.hook.as_ref().is_some_and(|h| h.unique_id == unique) {
                    self.hook = None;
                }
                self.loot.retain(|(_, u, _)| *u != unique);
            }
            // Both are frequent for mobs: only the hook's are decoded.
            EntityEvent::ID | SetEntityMotion::ID => {
                let Some(hook) = &mut self.hook else { return Ok(()) };
                if read_varint64(&mut &packet.body[..])? != hook.runtime_id {
                    return Ok(());
                }
                if packet.id == EntityEvent::ID {
                    let event = packet.decode::<EntityEvent>()?.event_id;
                    tracing::trace!(?event, "hook event");
                    if event == EntityEventEventId::FishHookHook {
                        hook.bites += 1;
                        tracing::debug!(bites = hook.bites, "hook event: bite");
                    }
                } else {
                    let vy = packet.decode::<SetEntityMotion>()?.velocity.y;
                    if hook.resting_updates >= SETTLED_UPDATES && vy < -DIP_SPEED {
                        hook.bites += 1;
                        tracing::debug!(bites = hook.bites, vy, "hook dip: bite");
                    }
                    hook.resting_updates = if vy.abs() < RESTING_SPEED { hook.resting_updates + 1 } else { 0 };
                }
            }
            ChangeDimension::ID => *self = Fishing::default(),
            _ => {}
        }
        Ok(())
    }
}

fn owner(p: &AddEntity) -> Option<i64> {
    p.metadata.iter().find(|m| m.key == MetadataDictionaryItemKey::OwnerEid).and_then(|m| match m.value {
        MetadataDictionaryItemValue::Default(MetadataDictionaryItemValueDefault::Long(id)) => Some(id),
        _ => None,
    })
}

#[cfg(test)]
mod tests;
