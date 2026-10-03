//! Movement-sensitive blocks (bedsim `block_effects.go`, `bubble.go`).

use crate::collide::inside_cells;
use crate::constants::{FREEZE_GAIN, FREEZE_LOSS};
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

/// Applies the queued cobweb/berry-bush/powder-snow multiplier once; true when one was applied.
pub(crate) fn apply_stuck_speed_multiplier(st: &mut PlayerState) -> bool {
    let mut m = st.stuck_speed_multiplier;
    if len_sqr(m) <= 1e-7 {
        return false;
    }
    // BDS: Weaving replaces whatever was queued, not only a cobweb's.
    if st.effects.weaving {
        m = [0.5, 0.25, 0.5];
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
            if st.pressing_descend {
                v[1] = -0.15;
                st.set_vel(v);
                return true;
            } else if st.pressing_ascend {
                v[1] = 0.15;
            }
        }
        Traversal::PowderSnow => {
            // No sneak descent: BDS sinks a sneaker at plain gravity (bedsim's -0.15 is not there).
            if st.pressing_ascend && st.equipment.leather_boots {
                v[1] = 0.2;
            }
        }
        Traversal::None => {}
    }
    st.set_vel(v);
    false
}

impl<W: WorldView + ?Sized> Sim<'_, W> {
    /// Powder snow freezing and its slowdown, before travel.
    // TODO: immunity (BDS skips freezing for some players; Java: any leather armour).
    pub(crate) fn update_freeze(&self, st: &mut PlayerState) {
        let in_snow = inside_cells(&st.bounding_box()).any(|pos| self.w.block(pos).inside == InsideMovement::PowderSnow);
        let stepped = if in_snow { (st.freeze + FREEZE_GAIN).min(1.0) } else { (st.freeze - FREEZE_LOSS).max(0.0) };
        let freeze = st.server_freeze.take().unwrap_or(stepped);
        if freeze != st.freeze {
            st.freeze = freeze;
            st.refresh_movement_speed();
        }
    }

    pub(crate) fn apply_inside_block_effects(&self, st: &mut PlayerState) {
        for pos in inside_cells(&st.bounding_box()) {
            let b = self.w.block(pos);
            if b.air {
                continue;
            }
            if b.cobweb {
                queue_stuck_speed_multiplier(st, [0.25, 0.05, 0.25]);
            }
            match b.inside {
                InsideMovement::SweetBerryBush => queue_stuck_speed_multiplier(st, [0.8, 0.75, 0.8]),
                InsideMovement::PowderSnow => queue_stuck_speed_multiplier(st, [0.9, 1.5, 0.9]),
                InsideMovement::None => {}
            }
        }
    }

    /// Slows the player once per honey block whose side it touches (not in water, see README).
    pub(crate) fn apply_honey_wall_slide(&self, st: &mut PlayerState) {
        // An inside-block effect: honey's box is inset 1/16, so a box against it is in its cell, while one
        // flush against a full block beside it is not. (Contact with the inset box itself is too strict: a
        // box walking along a honey wall is slowed by each cell it is in.)
        for pos in inside_cells(&st.bounding_box()) {
            if !self.w.block(pos).honey {
                continue;
            }
            // Any contact with its side, airborne or not, rising or falling (strict BDS fuzz: 0.4 per
            // touched honey cell on the ground too, as bedsim); not from on top of it.
            if st.pos[1] > pos[1] as f32 + 0.9375 - 1e-7 {
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
        let mut found = false;
        for pos in inside_cells(&st.bounding_box()) {
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
