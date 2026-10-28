//! Routing the entity packets to the tracked entities.

use acacia_client::proto::packets::{
    AddEntity, AddItemEntity, AddPlayer, ChangeDimension, MobArmorEquipment, MobEffect, MobEquipment, MoveEntity,
    MoveEntityDelta, MovePlayer, RemoveEntity, SetEntityData, SetEntityMotion, SyncEntityProperty, UpdateAttributes,
};
use acacia_client::proto::{DecodeError, Packet, RawPacket};

use super::{Entities, Entity, Equipment, ITEM_KIND, PLAYER_KIND};
use crate::state::Me;

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
        SyncEntityProperty::ID,
    ];

    /// The lean subset: spawns, moves, motion, metadata and removal.
    pub const LEAN_PACKETS: &'static [u32] = &[
        AddPlayer::ID,
        AddEntity::ID,
        RemoveEntity::ID,
        MoveEntity::ID,
        MoveEntityDelta::ID,
        MovePlayer::ID,
        SetEntityMotion::ID,
        SetEntityData::ID,
        ChangeDimension::ID,
    ];

    pub fn apply(&mut self, packet: &RawPacket, me: &Me) -> Result<(), DecodeError> {
        let now = self.now;
        match packet.id {
            MoveEntityDelta::ID => self.move_delta(&packet.decode()?),
            MoveEntity::ID => self.move_absolute(&packet.decode()?),
            MovePlayer::ID => self.move_player(&packet.decode()?),
            SetEntityMotion::ID => {
                let p: SetEntityMotion = packet.decode()?;
                if let Some(e) = self.by_runtime.get_mut(&p.runtime_entity_id) {
                    e.motion = p.velocity.clone();
                    e.velocity = p.velocity;
                }
            }
            SetEntityData::ID => {
                // Peek the leading runtime id: most metadata is for entities out of view.
                if let Some(e) = crate::state::leading_entity(packet).and_then(|id| self.by_runtime.get_mut(&id)) {
                    let p = packet.decode::<SetEntityData>()?;
                    if self.lean {
                        e.metadata.merge_lean(p.metadata);
                    } else {
                        e.metadata.merge(p.metadata);
                        e.properties.merge(p.properties);
                    }
                }
            }
            SyncEntityProperty::ID if !self.lean => self.property_defs.apply(&packet.decode()?)?,
            UpdateAttributes::ID if !self.lean => {
                if let Some(e) = crate::state::leading_entity(packet).and_then(|id| self.by_runtime.get_mut(&id)) {
                    e.attributes.extend(packet.decode::<UpdateAttributes>()?.attributes.into_iter().map(|a| (a.name, a.current)));
                }
            }
            MobEffect::ID if !self.lean => {
                let p: MobEffect = packet.decode()?;
                if let Some(e) = self.by_runtime.get_mut(&p.runtime_entity_id) {
                    e.effects.apply(&p);
                }
            }
            MobEquipment::ID if !self.lean => {
                let p: MobEquipment = packet.decode()?;
                if let Some(e) = self.by_runtime.get_mut(&p.runtime_entity_id) {
                    e.equipment.get_or_insert_default().apply_hand(p);
                }
            }
            MobArmorEquipment::ID if !self.lean => {
                let p: MobArmorEquipment = packet.decode()?;
                if let Some(e) = self.by_runtime.get_mut(&p.runtime_entity_id) {
                    e.equipment.get_or_insert_default().apply_armor(p);
                }
            }
            AddPlayer::ID => {
                let p: AddPlayer = packet.decode()?;
                let mut e = Entity::spawned(p.runtime_id, p.unique_id, PLAYER_KIND.to_owned(), p.position, p.velocity);
                (e.yaw, e.pitch, e.head_yaw, e.moved_at) = (p.yaw, p.pitch, p.head_yaw, now);
                e.username = Some(p.username);
                e.uuid = Some(p.uuid);
                if self.lean {
                    e.metadata.merge_lean(p.metadata);
                } else {
                    e.metadata = p.metadata.into();
                    e.equipment = Equipment::holding(p.held_item);
                }
                self.insert(me, e);
            }
            AddEntity::ID => {
                let p: AddEntity = packet.decode()?;
                let mut e = Entity::spawned(p.runtime_id, p.unique_id, p.entity_type, p.position, p.velocity);
                (e.yaw, e.pitch, e.head_yaw, e.moved_at) = (p.yaw, p.pitch, p.head_yaw, now);
                if self.lean {
                    e.metadata.merge_lean(p.metadata);
                } else {
                    e.metadata = p.metadata.into();
                    e.properties.merge(p.properties);
                    e.attributes = p.attributes.into_iter().map(|a| (a.name, a.value)).collect();
                }
                self.insert(me, e);
            }
            AddItemEntity::ID if !self.lean => {
                let p: AddItemEntity = packet.decode()?;
                let mut e = Entity::spawned(p.runtime_entity_id, p.entity_id_self, ITEM_KIND.to_owned(), p.position, p.velocity);
                e.moved_at = now;
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
        self.by_runtime.insert(entity.runtime_id, Box::new(entity));
    }
}
