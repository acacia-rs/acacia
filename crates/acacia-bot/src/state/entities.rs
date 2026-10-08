use std::collections::HashMap;

use acacia_client::proto::packets::{
    AddEntity, AddItemEntity, AddPlayer, ChangeDimension, MobArmorEquipment, MobEffect, MobEquipment, MoveEntity,
    MoveEntityDelta, MovePlayer, RemoveEntity, SetEntityData, SetEntityMotion, UpdateAttributes,
};
use acacia_client::proto::manual::Uuid;
use acacia_client::proto::types::Vec3f;
use acacia_client::proto::{DecodeError, Packet, RawPacket};

use super::{Effects, ItemStack, Me, Metadata};

mod equipment;

pub use equipment::Equipment;

pub const PLAYER_KIND: &str = "minecraft:player";
pub const ITEM_KIND: &str = "minecraft:item";
/// Players' wire position is this far above their feet, whatever their pose.
pub const PLAYER_EYE_HEIGHT: f32 = super::PlayerState::EYE_HEIGHT;

const BYTE_ROTATION: f32 = 360.0 / 256.0;

/// One tracked entity or other player. Angles are in degrees.
#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub runtime_id: u64,
    pub unique_id: i64,
    /// Identifier such as `minecraft:zombie`; [`PLAYER_KIND`] for players, [`ITEM_KIND`] for dropped items.
    pub kind: String,
    pub username: Option<String>,
    /// Position as sent by the server: the eye position for players (see [`Entity::feet`]), feet otherwise.
    pub position: Vec3f,
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    /// Last velocity from spawn or SetEntityMotion; not derived from movement.
    pub velocity: Vec3f,
    pub on_ground: bool,
    /// Players only; the key into [`super::PlayerList`] and [`super::Skins`].
    pub uuid: Option<Uuid>,
    pub metadata: Metadata,
    /// Current attribute values by name (`minecraft:health`, `minecraft:movement`, ...).
    pub attributes: HashMap<String, f32>,
    pub effects: Effects,
    /// `None` until the server sends any.
    pub equipment: Option<Box<Equipment>>,
    /// The stack of a dropped item ([`ITEM_KIND`]).
    pub item: Option<ItemStack>,
}

impl Entity {
    fn spawned(runtime_id: u64, unique_id: i64, kind: String, position: Vec3f, velocity: Vec3f) -> Self {
        Entity {
            runtime_id,
            unique_id,
            kind,
            username: None,
            position,
            yaw: 0.0,
            pitch: 0.0,
            head_yaw: 0.0,
            velocity,
            on_ground: false,
            uuid: None,
            metadata: Metadata::default(),
            attributes: HashMap::new(),
            effects: Effects::default(),
            equipment: None,
            item: None,
        }
    }

    pub fn is_player(&self) -> bool {
        self.kind == PLAYER_KIND
    }

    pub fn feet(&self) -> Vec3f {
        let p = &self.position;
        let dy = if self.is_player() { PLAYER_EYE_HEIGHT } else { 0.0 };
        Vec3f { x: p.x, y: p.y - dy, z: p.z }
    }

    pub fn distance_sq(&self, to: &Vec3f) -> f32 {
        let (dx, dy, dz) = (self.position.x - to.x, self.position.y - to.y, self.position.z - to.z);
        dx * dx + dy * dy + dz * dz
    }

    /// `minecraft:health` if the server sent it (mobs only; players' health is not broadcast).
    pub fn health(&self) -> Option<f32> {
        self.attributes.get("minecraft:health").copied()
    }
}

/// Entities by runtime id, excluding the local player. Off by default (see [`crate::BotConfig`]).
#[derive(Debug, Default)]
pub struct Entities {
    by_runtime: HashMap<u64, Entity>,
    runtime_by_unique: HashMap<i64, u64>,
}

impl Entities {
    pub const PACKETS: &'static [u32] = &[
        AddPlayer::ID,
        AddEntity::ID,
        AddItemEntity::ID,
        UpdateAttributes::ID,
        RemoveEntity::ID,
        MoveEntity::ID,
        MoveEntityDelta::ID,
        MovePlayer::ID,
        SetEntityMotion::ID,
        SetEntityData::ID,
        MobEffect::ID,
        MobEquipment::ID,
        MobArmorEquipment::ID,
        ChangeDimension::ID,
    ];

    pub fn get(&self, runtime_id: u64) -> Option<&Entity> {
        self.by_runtime.get(&runtime_id)
    }

    /// Moves a tracked entity the bot itself simulates (a vehicle it drives, which the server does not
    /// send moves for).
    pub(crate) fn set_pose(&mut self, runtime_id: u64, feet: [f32; 3], yaw: f32, pitch: f32) {
        if let Some(e) = self.by_runtime.get_mut(&runtime_id) {
            let [x, y, z] = feet;
            e.position = Vec3f { x, y, z };
            (e.yaw, e.pitch) = (yaw, pitch);
        }
    }

    pub fn by_unique(&self, unique_id: i64) -> Option<&Entity> {
        self.runtime_by_unique.get(&unique_id).and_then(|r| self.by_runtime.get(r))
    }

    pub fn len(&self) -> usize {
        self.by_runtime.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_runtime.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.by_runtime.values()
    }

    pub fn players(&self) -> impl Iterator<Item = &Entity> {
        self.iter().filter(|e| e.is_player())
    }

    /// Closest entity to `pos` (by wire position) that passes `filter`.
    pub fn nearest(&self, pos: &Vec3f, filter: impl Fn(&Entity) -> bool) -> Option<&Entity> {
        self.iter().filter(|e| filter(e)).min_by(|a, b| a.distance_sq(pos).total_cmp(&b.distance_sq(pos)))
    }

    pub fn apply(&mut self, packet: &RawPacket, me: &Me) -> Result<(), DecodeError> {
        match packet.id {
            MoveEntityDelta::ID => self.move_delta(&packet.decode()?),
            MoveEntity::ID => self.move_absolute(&packet.decode()?),
            MovePlayer::ID => self.move_player(&packet.decode()?),
            SetEntityMotion::ID => {
                let p: SetEntityMotion = packet.decode()?;
                if let Some(e) = self.by_runtime.get_mut(&p.runtime_entity_id) {
                    e.velocity = p.velocity;
                }
            }
            SetEntityData::ID => {
                // Peek the leading runtime id: most metadata is for entities out of view.
                let id = acacia_client::proto::codec::read_varint64(&mut &packet.body[..])?;
                if let Some(e) = self.by_runtime.get_mut(&id) {
                    e.metadata.merge(packet.decode::<SetEntityData>()?.metadata);
                }
            }
            UpdateAttributes::ID => {
                let p: UpdateAttributes = packet.decode()?;
                if let Some(e) = self.by_runtime.get_mut(&p.runtime_entity_id) {
                    e.attributes.extend(p.attributes.into_iter().map(|a| (a.name, a.current)));
                }
            }
            MobEffect::ID => {
                let p: MobEffect = packet.decode()?;
                if let Some(e) = self.by_runtime.get_mut(&p.runtime_entity_id) {
                    e.effects.apply(&p);
                }
            }
            MobEquipment::ID => {
                let p: MobEquipment = packet.decode()?;
                if let Some(e) = self.by_runtime.get_mut(&p.runtime_entity_id) {
                    e.equipment.get_or_insert_default().apply_hand(p);
                }
            }
            MobArmorEquipment::ID => {
                let p: MobArmorEquipment = packet.decode()?;
                if let Some(e) = self.by_runtime.get_mut(&p.runtime_entity_id) {
                    e.equipment.get_or_insert_default().apply_armor(p);
                }
            }
            AddPlayer::ID => {
                let p: AddPlayer = packet.decode()?;
                let mut e = Entity::spawned(p.runtime_id, p.unique_id, PLAYER_KIND.to_owned(), p.position, p.velocity);
                (e.yaw, e.pitch, e.head_yaw) = (p.yaw, p.pitch, p.head_yaw);
                e.username = Some(p.username);
                e.uuid = Some(p.uuid);
                e.metadata = p.metadata.into();
                e.equipment = Equipment::holding(p.held_item);
                self.insert(me, e);
            }
            AddEntity::ID => {
                let p: AddEntity = packet.decode()?;
                let mut e = Entity::spawned(p.runtime_id, p.unique_id, p.entity_type, p.position, p.velocity);
                (e.yaw, e.pitch, e.head_yaw) = (p.yaw, p.pitch, p.head_yaw);
                e.metadata = p.metadata.into();
                e.attributes = p.attributes.into_iter().map(|a| (a.name, a.value)).collect();
                self.insert(me, e);
            }
            AddItemEntity::ID => {
                let p: AddItemEntity = packet.decode()?;
                let mut e = Entity::spawned(p.runtime_entity_id, p.entity_id_self, ITEM_KIND.to_owned(), p.position, p.velocity);
                e.metadata = p.metadata.into();
                e.item = Some(p.item.into());
                self.insert(me, e);
            }
            RemoveEntity::ID => {
                let unique = packet.decode::<RemoveEntity>()?.entity_id_self;
                if let Some(runtime) = self.runtime_by_unique.remove(&unique) {
                    self.by_runtime.remove(&runtime);
                }
            }
            // The client drops every entity on a dimension change; the server re-adds what is in view.
            ChangeDimension::ID => {
                self.by_runtime.clear();
                self.runtime_by_unique.clear();
            }
            _ => {}
        }
        Ok(())
    }

    fn insert(&mut self, me: &Me, entity: Entity) {
        if entity.runtime_id == me.runtime_entity_id {
            return;
        }
        if let Some(old) = self.by_runtime.get(&entity.runtime_id) {
            self.runtime_by_unique.remove(&old.unique_id);
        }
        self.runtime_by_unique.insert(entity.unique_id, entity.runtime_id);
        self.by_runtime.insert(entity.runtime_id, entity);
    }

    // Hot path (~300/s on busy servers): no allocation. Despite the name, all fields are absolute.
    fn move_delta(&mut self, p: &MoveEntityDelta) {
        let Some(e) = self.by_runtime.get_mut(&p.runtime_entity_id) else { return };
        if let Some(x) = p.x {
            e.position.x = x;
        }
        if let Some(y) = p.y {
            e.position.y = y;
        }
        if let Some(z) = p.z {
            e.position.z = z;
        }
        if let Some(r) = p.rot_x {
            e.pitch = r as f32 * BYTE_ROTATION;
        }
        if let Some(r) = p.rot_y {
            e.yaw = r as f32 * BYTE_ROTATION;
        }
        if let Some(r) = p.rot_z {
            e.head_yaw = r as f32 * BYTE_ROTATION;
        }
        e.on_ground = p.on_ground;
    }

    fn move_absolute(&mut self, p: &MoveEntity) {
        const ON_GROUND: u8 = 1;
        let Some(e) = self.by_runtime.get_mut(&p.runtime_entity_id) else { return };
        e.position = p.position.clone();
        // Generated field names are off: the wire order is pitch, yaw, head yaw (gophertunnel, PMMP).
        e.pitch = p.rotation.yaw;
        e.yaw = p.rotation.pitch;
        e.head_yaw = p.rotation.head_yaw;
        e.on_ground = p.flags & ON_GROUND != 0;
    }

    fn move_player(&mut self, p: &MovePlayer) {
        let Some(e) = self.by_runtime.get_mut(&(p.runtime_id as u64)) else { return };
        e.position = p.position.clone();
        e.pitch = p.pitch;
        e.yaw = p.yaw;
        e.head_yaw = p.head_yaw;
        e.on_ground = p.on_ground;
    }
}

#[cfg(test)]
mod tests;
