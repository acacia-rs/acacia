//! Where BDS puts a passenger who leaves a vehicle with `dismount_mode` default (pigs, boats, horses):
//! matched live against BDS for pig and boat (docs/research/riding-fishing-elytra.md).

use acacia_physics::{Aabb, Vec3, WorldView};

use super::input::vehicle_pose;
use crate::state::GameState;

const PLAYER_WIDTH: f32 = 0.6;
const PLAYER_HEIGHT: f32 = 1.8;
/// Squared horizontal speed (blocks/tick) up to which the vehicle counts as standing still.
const STILL_SQ: f32 = 9.801e-5;
/// Added to the exit's feet height.
const LIFT: f32 = 0.001;

/// The vehicle pose and seat the exit search starts from.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Mount {
    /// The vehicle's position (feet).
    pub vehicle: Vec3,
    /// The vehicle's yaw in degrees.
    pub yaw: f32,
    /// The vehicle's horizontal motion per tick (x, z).
    pub motion: [f32; 2],
    /// The rider's seat offset in the vehicle's frame (`RiderSeatPosition`).
    pub seat_offset: Vec3,
}

impl Mount {
    pub(crate) fn of(state: &GameState) -> Option<Mount> {
        let vehicle = state.riding.vehicle.as_ref()?;
        let pose = vehicle_pose(state, vehicle)?;
        let velocity = vehicle.runtime_id.and_then(|id| state.entities.get(id)).map(|e| [e.velocity.x, e.velocity.z]);
        let o = state.riding.seat_offset.as_ref().map_or([0.0; 3], |o| [o.x, o.y, o.z]);
        let p = &pose.position;
        Some(Mount { vehicle: [p.x, p.y, p.z], yaw: pose.yaw, motion: velocity.unwrap_or([0.0; 2]), seat_offset: o })
    }

    /// The seat in world space: the offset turned by the vehicle's yaw.
    pub(crate) fn seat(&self) -> Vec3 {
        let (v, o) = (self.vehicle, self.seat_offset);
        let (sin, cos) = (-self.yaw.to_radians()).sin_cos();
        [v[0] + o[2] * sin + o[0] * cos, v[1] + o[1], v[2] + o[2] * cos - o[0] * sin]
    }

    /// Feet of the passenger after leaving: the first free spot of 24 around the vehicle's block, at
    /// the seat's x/z. `None` when every spot is blocked (BDS then leaves the passenger where it is).
    pub(crate) fn exit_feet(&self, world: &impl WorldView) -> Option<Vec3> {
        let v = self.vehicle;
        let centre = [v[0].floor() + 0.5, v[1].floor(), v[2].floor() + 0.5];
        let seat = self.seat();
        for dy in [0.0, 1.0, -1.0] {
            for [hx, hz] in horizontal_offsets(self.motion) {
                let spot = [centre[0] + hx, centre[1] + dy, centre[2] + hz];
                if let Some(floor) = free_spot(world, spot) {
                    return Some([seat[0] + hx, v[1].floor() + floor + dy + LIFT, seat[2] + hz]);
                }
            }
        }
        None
    }
}

/// The eight horizontal offsets in search order, from the vehicle's dominant direction of motion.
fn horizontal_offsets([dx, dz]: [f32; 2]) -> [[f32; 2]; 8] {
    let f = if dx * dx + dz * dz <= STILL_SQ {
        [-1.0, 0.0]
    } else if dx.abs() > dz.abs() {
        [dx.signum(), 0.0]
    } else {
        [0.0, dz.signum()]
    };
    let s = [-f[1], f[0]];
    let add = |a: [f32; 2], b: [f32; 2], k: f32| [a[0] + k * b[0], a[1] + k * b[1]];
    let neg = |a: [f32; 2]| [-a[0], -a[1]];
    [s, neg(s), add(s, f, -1.0), add(neg(s), f, -1.0), add(s, f, 1.0), add(f, s, -1.0), neg(f), f]
}

/// The floor height inside the spot's block if a player fits there, standing on it.
fn free_spot(world: &impl WorldView, p: Vec3) -> Option<f32> {
    let cell = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
    let floor = match top(world, cell) {
        Some(t) if t > 0.0 && t < 1.0 => t,
        Some(_) => return None,
        None => {
            let below = top(world, [cell[0], cell[1] - 1, cell[2]])? - 1.0;
            if below < 0.0 {
                return None;
            }
            below
        }
    };
    let half = PLAYER_WIDTH.min(1.0) / 2.0;
    let body = Aabb::new(p[0] - half, p[1] + floor, p[2] - half, p[0] + half, p[1] + floor + PLAYER_HEIGHT, p[2] + half);
    let mut hits = Vec::new();
    world.collisions(&body, &mut hits);
    hits.is_empty().then_some(floor)
}

/// Height of the block's highest collision box above its base, `None` without boxes.
fn top(world: &impl WorldView, cell: [i32; 3]) -> Option<f32> {
    let mut boxes = Vec::new();
    world.block_collisions(cell, &mut boxes);
    boxes.iter().map(|b| b.max[1]).reduce(f32::max)
}

#[cfg(test)]
mod tests;
