//! Movement-sensitive blocks (bedsim `block_effects.go`, `bubble.go`).

use crate::collide::overlapped_cells;
use crate::math::{Vec3, len_sqr};
use crate::sim::Sim;
use crate::state::PlayerState;
use crate::world::{InsideMovement, Traversal, WorldView};

fn queue_stuck_speed_multiplier(st: &mut PlayerState, m: Vec3) {
    let mut q = st.stuck_speed_multiplier;
    if len_sqr(q) <= 1e-7 {
        st.stuck_speed_multiplier = m;
        return;
    }
    for i in 0..3 {
        q[i] = q[i].min(m[i]);
    }
    st.stuck_speed_multiplier = q;
}

/// Applies the queued berry-bush/powder-snow multiplier once; true when one was applied.
pub(crate) fn apply_stuck_speed_multiplier(st: &mut PlayerState) -> bool {
    let m = st.stuck_speed_multiplier;
    if len_sqr(m) <= 1e-7 {
        return false;
    }
    st.set_vel([st.vel[0] * m[0], st.vel[1] * m[1], st.vel[2] * m[2]]);
    st.stuck_speed_multiplier = [0.0; 3];
    true
}

/// Scaffolding/powder-snow vertical traversal; true when ordinary vertical travel is skipped.
pub(crate) fn apply_ascendable_movement(st: &mut PlayerState, traversal: Traversal) -> bool {
    let mut v = st.vel;
    match traversal {
        Traversal::Scaffolding => {
            // On the ground below, sneaking inside scaffolding leaves ordinary gravity (BDS fuzz).
            if st.pressing_descend && !st.on_ground {
                v[1] = -0.15;
                st.set_vel(v);
                return true;
            } else if st.pressing_ascend {
                v[1] = 0.15;
            }
        }
        // Only boots make powder snow walkable; without them sneaking leaves the sink to gravity (BDS fuzz: a
        // sneaking jump in powder snow still rises).
        Traversal::PowderSnow if st.equipment.leather_boots => {
            if st.pressing_descend {
                v[1] = -0.15;
            } else if st.pressing_ascend {
                v[1] = 0.2;
            }
        }
        Traversal::PowderSnow => {}
        Traversal::None => {}
    }
    st.set_vel(v);
    false
}

impl<W: WorldView + ?Sized> Sim<'_, W> {
    pub(crate) fn apply_inside_block_effects(&self, st: &mut PlayerState) {
        let bb = st.bounding_box();
        for pos in overlapped_cells(&bb) {
            if !bb.intersects(&crate::aabb::Aabb::block(pos)) {
                continue;
            }
            let b = self.w.block(pos);
            if b.air {
                continue;
            }
            match b.inside {
                InsideMovement::SweetBerryBush => queue_stuck_speed_multiplier(st, [0.8, 0.75, 0.8]),
                InsideMovement::PowderSnow => queue_stuck_speed_multiplier(st, [0.9, 1.5, 0.9]),
                InsideMovement::None => {}
            }
        }
        self.apply_honey_wall_slide(st);
    }

    /// Slows the player once per overlapped honey block it is sliding down.
    fn apply_honey_wall_slide(&self, st: &mut PlayerState) {
        let bb = st.bounding_box().grow_vec([1e-3, 0.0, 1e-3]);
        for pos in overlapped_cells(&bb) {
            if !bb.intersects(&crate::aabb::Aabb::block(pos)) || !self.w.block(pos).honey {
                continue;
            }
            // Only while sliding down its side: airborne, falling, below its top (BDS, as Java; bedsim
            // slows on any contact).
            if st.on_ground || st.vel[1] >= -0.08 || st.pos[1] > pos[1] as f32 + 0.9375 - 1e-7 {
                continue;
            }
            let mut v = st.vel;
            v[0] *= 0.4;
            v[1] = (-0.12f32).max(v[1]);
            v[2] *= 0.4;
            st.set_vel(v);
            if honey_slide_resets_fall_distance(st, pos) {
                st.fall_distance = 0.0;
            }
        }
    }

    pub(crate) fn apply_bubble_columns(&self, st: &mut PlayerState) {
        let bb = st.bounding_box();
        let mut found = false;
        for pos in overlapped_cells(&bb) {
            let Some(col) = self.w.block(pos).bubble_column else { continue };
            found = true;
            let surface = col.surface.unwrap_or_else(|| {
                let above = self.w.block([pos[0], pos[1] + 1, pos[2]]);
                above.liquid.is_none() && above.air
            });
            let mut v = st.vel;
            if col.downward {
                let cap = if surface { -0.9 } else { -0.3 };
                v[1] = f32::max(cap, v[1] - 0.03);
            } else {
                let (change, cap) = if surface { (0.1, 1.8) } else { (0.06, 0.7) };
                v[1] = f32::min(cap, v[1] + change);
            }
            st.set_vel(v);
        }
        if found {
            st.fall_distance = 0.0;
        }
    }
}

/// Contact with a honey side rather than its top surface.
fn honey_slide_resets_fall_distance(st: &PlayerState, pos: [i32; 3]) -> bool {
    if st.vel[1] >= 0.0 || st.pos[1] > pos[1] as f32 + 0.9375 {
        return false;
    }
    let radius = st.size[0] * st.size[2] * 0.5 + 0.43125;
    let (cx, cz) = (pos[0] as f32 + 0.5, pos[2] as f32 + 0.5);
    (cx - st.pos[0]).abs() > radius || (cz - st.pos[2]).abs() > radius
}
