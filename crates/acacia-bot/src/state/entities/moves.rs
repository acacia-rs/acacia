//! The move packets' updates to tracked entities.

use acacia_client::proto::packets::{MoveEntity, MoveEntityDelta, MovePlayer};

use super::{BYTE_ROTATION, Entities};

impl Entities {
    // Hot path (~300/s on busy servers): no allocation. Despite the name, all fields are absolute.
    pub(super) fn move_delta(&mut self, p: &MoveEntityDelta) {
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

    pub(super) fn move_absolute(&mut self, p: &MoveEntity) {
        const ON_GROUND: u8 = 1;
        let Some(e) = self.by_runtime.get_mut(&p.runtime_entity_id) else { return };
        e.position = p.position.clone();
        // Generated field names are off: the wire order is pitch, yaw, head yaw (gophertunnel, PMMP).
        e.pitch = p.rotation.yaw;
        e.yaw = p.rotation.pitch;
        e.head_yaw = p.rotation.head_yaw;
        e.on_ground = p.flags & ON_GROUND != 0;
    }

    pub(super) fn move_player(&mut self, p: &MovePlayer) {
        let Some(e) = self.by_runtime.get_mut(&(p.runtime_id as u64)) else { return };
        e.position = p.position.clone();
        e.pitch = p.pitch;
        e.yaw = p.yaw;
        e.head_yaw = p.head_yaw;
        e.on_ground = p.on_ground;
    }
}
