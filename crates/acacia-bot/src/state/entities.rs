use std::collections::HashMap;

use acacia_client::proto::packets::{
    AddEntity, AddItemEntity, AddPlayer, ChangeDimension, MoveEntity, MoveEntityDelta, MovePlayer, RemoveEntity,
    SetEntityMotion,
};
use acacia_client::proto::types::Vec3f;
use acacia_client::proto::{DecodeError, Packet, RawPacket};

use super::Me;

pub const PLAYER_KIND: &str = "minecraft:player";
pub const ITEM_KIND: &str = "minecraft:item";
/// Players' wire position is this far above their feet, whatever their pose.
pub const PLAYER_EYE_HEIGHT: f32 = 1.62;

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
}

impl Entity {
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
        RemoveEntity::ID,
        MoveEntity::ID,
        MoveEntityDelta::ID,
        MovePlayer::ID,
        SetEntityMotion::ID,
        ChangeDimension::ID,
    ];

    pub fn get(&self, runtime_id: u64) -> Option<&Entity> {
        self.by_runtime.get(&runtime_id)
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
            AddPlayer::ID => {
                let p: AddPlayer = packet.decode()?;
                self.insert(me, Entity {
                    runtime_id: p.runtime_id,
                    unique_id: p.unique_id,
                    kind: PLAYER_KIND.to_owned(),
                    username: Some(p.username),
                    position: p.position,
                    yaw: p.yaw,
                    pitch: p.pitch,
                    head_yaw: p.head_yaw,
                    velocity: p.velocity,
                    on_ground: false,
                });
            }
            AddEntity::ID => {
                let p: AddEntity = packet.decode()?;
                self.insert(me, Entity {
                    runtime_id: p.runtime_id,
                    unique_id: p.unique_id,
                    kind: p.entity_type,
                    username: None,
                    position: p.position,
                    yaw: p.yaw,
                    pitch: p.pitch,
                    head_yaw: p.head_yaw,
                    velocity: p.velocity,
                    on_ground: false,
                });
            }
            AddItemEntity::ID => {
                let p: AddItemEntity = packet.decode()?;
                self.insert(me, Entity {
                    runtime_id: p.runtime_entity_id,
                    unique_id: p.entity_id_self,
                    kind: ITEM_KIND.to_owned(),
                    username: None,
                    position: p.position,
                    yaw: 0.0,
                    pitch: 0.0,
                    head_yaw: 0.0,
                    velocity: p.velocity,
                    on_ground: false,
                });
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
