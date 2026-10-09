use std::collections::HashMap;

use acacia_client::proto::codec::read_varint64;
use acacia_client::proto::packets::{AddEntity, AddPlayer, ChangeDimension, RemoveEntity, SetEntityData, SetEntityLink};
use acacia_client::proto::types::{Link, MetadataDictionaryItemKey, MetadataDictionaryItemValue, MetadataDictionaryItemValueDefault, Vec3f};
use acacia_client::proto::{DecodeError, Packet, RawPacket};

use super::Me;

/// `EntityLink` types (gophertunnel `EntityLinkRemove`/`Rider`/`Passenger`).
const LINK_REMOVE: u8 = 0;
const LINK_RIDER: u8 = 1;

/// The entity the local player sits on.
#[derive(Debug, Clone, PartialEq)]
pub struct Vehicle {
    pub unique_id: i64,
    /// `None` when the vehicle's spawn was not seen (it was a player, or spawned before tracking).
    pub runtime_id: Option<u64>,
    /// Identifier such as `minecraft:boat`, when known.
    pub kind: Option<String>,
    /// In the controlling seat (link type Rider) rather than a passenger seat.
    pub driver: bool,
}

/// Where and how an entity spawned (wire position, degrees).
#[derive(Debug, Clone, PartialEq)]
pub struct Pose {
    pub position: Vec3f,
    pub pitch: f32,
    pub yaw: f32,
}

#[derive(Debug)]
struct Spawned {
    runtime_id: u64,
    kind: String,
    pose: Pose,
}

/// What the local player rides, from `SetEntityLink` and the links in `AddEntity`. Always on: it keeps
/// ids, identifiers and spawn poses only, so riding works without `Trackers::entities` (which adds the
/// vehicle's later moves).
#[derive(Debug, Default)]
pub struct Riding {
    pub vehicle: Option<Vehicle>,
    /// The own seat offset from the vehicle (`SetEntityData` RiderSeatPosition), while riding.
    pub seat_offset: Option<Vec3f>,
    spawned: HashMap<i64, Spawned>,
    /// Every linked rider's vehicle, by unique ids (the local player's included).
    links: HashMap<i64, i64>,
}

impl Riding {
    pub const PACKETS: &'static [u32] =
        &[AddEntity::ID, AddPlayer::ID, RemoveEntity::ID, SetEntityLink::ID, SetEntityData::ID, ChangeDimension::ID];

    pub fn is_riding(&self) -> bool {
        self.vehicle.is_some()
    }

    /// Runtime id of a spawned entity (players excluded) from its unique id.
    pub fn runtime_id_of(&self, unique_id: i64) -> Option<u64> {
        self.spawned.get(&unique_id).map(|s| s.runtime_id)
    }

    /// The pose an entity spawned with.
    pub fn spawn_pose(&self, unique_id: i64) -> Option<&Pose> {
        self.spawned.get(&unique_id).map(|s| &s.pose)
    }

    /// Unique id of the entity `rider` (a unique id) sits on.
    pub fn vehicle_of(&self, rider: i64) -> Option<i64> {
        self.links.get(&rider).copied()
    }

    /// Forgets the seat without waiting for the server, as the vanilla client does when it dismounts.
    pub(crate) fn leave(&mut self) {
        self.vehicle = None;
        self.seat_offset = None;
    }

    pub fn apply(&mut self, packet: &RawPacket, me: &Me) -> Result<(), DecodeError> {
        match packet.id {
            AddEntity::ID => {
                let p: AddEntity = packet.decode()?;
                let pose = Pose { position: p.position, pitch: p.pitch, yaw: p.yaw };
                self.spawned.insert(p.unique_id, Spawned { runtime_id: p.runtime_id, kind: p.entity_type, pose });
                p.links.iter().for_each(|l| self.on_link(l, me));
            }
            AddPlayer::ID => packet.decode::<AddPlayer>()?.links.iter().for_each(|l| self.on_link(l, me)),
            RemoveEntity::ID => {
                let unique = packet.decode::<RemoveEntity>()?.entity_id_self;
                self.spawned.remove(&unique);
                self.links.retain(|rider, ridden| *rider != unique && *ridden != unique);
                if self.vehicle.as_ref().is_some_and(|v| v.unique_id == unique) {
                    self.leave();
                }
            }
            SetEntityLink::ID => self.on_link(&packet.decode::<SetEntityLink>()?.link, me),
            // Every entity's metadata comes through here; only the own is decoded.
            SetEntityData::ID if read_varint64(&mut &packet.body[..])? == me.runtime_entity_id => {
                let seat = packet.decode::<SetEntityData>()?.metadata.into_iter().find_map(|m| match m.value {
                    MetadataDictionaryItemValue::Default(MetadataDictionaryItemValueDefault::Vec3f(v))
                        if m.key == MetadataDictionaryItemKey::RiderSeatPosition => Some(v),
                    _ => None,
                });
                if seat.is_some() {
                    self.seat_offset = seat;
                }
            }
            ChangeDimension::ID => {
                self.spawned.clear();
                self.links.clear();
                self.leave();
            }
            _ => {}
        }
        Ok(())
    }

    fn on_link(&mut self, link: &Link, me: &Me) {
        match link.r#type {
            LINK_REMOVE => self.links.retain(|rider, ridden| (*rider, *ridden) != (link.rider_entity_id, link.ridden_entity_id)),
            _ => _ = self.links.insert(link.rider_entity_id, link.ridden_entity_id),
        }
        if link.rider_entity_id != me.unique_entity_id {
            return;
        }
        if link.r#type == LINK_REMOVE {
            if self.vehicle.as_ref().is_some_and(|v| v.unique_id == link.ridden_entity_id) {
                self.leave();
            }
            return;
        }
        let known = self.spawned.get(&link.ridden_entity_id);
        self.vehicle = Some(Vehicle {
            unique_id: link.ridden_entity_id,
            runtime_id: known.map(|s| s.runtime_id),
            kind: known.map(|s| s.kind.clone()),
            driver: link.r#type == LINK_RIDER,
        });
    }
}

#[cfg(test)]
mod tests;
