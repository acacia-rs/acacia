//! The move packets' updates to tracked entities.

use acacia_client::proto::packets::{MoveEntity, MoveEntityDelta, MovePlayer, MovePlayerMode};
use acacia_client::proto::types::Vec3f;

use super::{BYTE_ROTATION, Entities, Entity};

/// `MoveEntity` flags.
const ON_GROUND: u8 = 1;
const TELEPORT: u8 = 2;

/// Puts `e` at `to`, deriving its motion over the ticks since its last move (none across a teleport).
fn relocate(e: &mut Entity, to: Vec3f, now: u64, teleport: bool) {
    if to == e.position {
        return;
    }
    let dt = now.saturating_sub(e.moved_at).max(1) as f32;
    e.motion = if teleport {
        Vec3f { x: 0.0, y: 0.0, z: 0.0 }
    } else {
        Vec3f { x: (to.x - e.position.x) / dt, y: (to.y - e.position.y) / dt, z: (to.z - e.position.z) / dt }
    };
    (e.position, e.moved_at) = (to, now);
}

impl Entities {
    // Hot path (~300/s on busy servers): no allocation. Despite the name, all fields are absolute.
    pub(super) fn move_delta(&mut self, p: &MoveEntityDelta) {
        let now = self.now;
        let Some(e) = self.by_runtime.get_mut(&p.runtime_entity_id) else { return };
        let old = &e.position;
        let to = Vec3f { x: p.x.unwrap_or(old.x), y: p.y.unwrap_or(old.y), z: p.z.unwrap_or(old.z) };
        relocate(e, to, now, false);
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

    pub(super) fn move_absolute(&mut self, p: &MoveEntity) {
        let now = self.now;
        let Some(e) = self.by_runtime.get_mut(&p.runtime_entity_id) else { return };
        relocate(e, p.position.clone(), now, p.flags & TELEPORT != 0);
        // Generated field names are off: the wire order is pitch, yaw, head yaw (gophertunnel, PMMP).
        e.pitch = p.rotation.yaw;
        e.yaw = p.rotation.pitch;
        e.head_yaw = p.rotation.head_yaw;
        e.on_ground = p.flags & ON_GROUND != 0;
    }

    pub(super) fn move_player(&mut self, p: &MovePlayer) {
        let now = self.now;
        let Some(e) = self.by_runtime.get_mut(&(p.runtime_id as u64)) else { return };
        let teleport = matches!(p.mode, MovePlayerMode::Reset | MovePlayerMode::Teleport);
        relocate(e, p.position.clone(), now, teleport);
        e.pitch = p.pitch;
        e.yaw = p.yaw;
        e.head_yaw = p.head_yaw;
        e.on_ground = p.on_ground;
    }
}
