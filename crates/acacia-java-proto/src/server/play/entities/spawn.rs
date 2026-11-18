//! `ClientboundAddEntityPacket` and `ClientboundRemoveEntitiesPacket`.

use crate::entity::LpVec3;
use crate::packets::packet;
use crate::{Error, VarInt, Wire};

// Angles are `pack_degrees` bytes; `data` is the entity type's spawn data (a hanging entity's `Direction` 3D value).
packet!(AddEntity {
    id: VarInt,
    uuid: u128,
    entity_type: VarInt,
    x: f64,
    y: f64,
    z: f64,
    movement: LpVec3,
    x_rot: u8,
    y_rot: u8,
    y_head_rot: u8,
    data: VarInt,
});

packet!(RemoveEntities { ids: Vec<VarInt> });
