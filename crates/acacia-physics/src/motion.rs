//! Pure velocity helpers of bedsim `simulation.go` (move relative, jump, glide, landing).

use crate::aabb::Aabb;
use crate::constants::*;
use crate::math::{PI32, Vec3, cos32, mc_cos, mc_sin, sin32};
use crate::state::PlayerState;
use crate::world::{BlockPhysics, Bounce};

/// Applies the input impulse as acceleration at `speed`.
pub(crate) fn move_relative(st: &mut PlayerState, speed: f32) {
    let i = st.impulse;
    let mut force = i[1] * i[1] + i[0] * i[0];
    if force >= 1e-4 {
        force = speed / force.sqrt().max(1.0);
        let mf = i[1] * force;
        let ms = i[0] * force;
        // DefaultMoveSystems::horizontalMovement uses sincosf, not the lookup table.
        let yaw = st.yaw * (PI32 / 180.0);
        let (v2, v3) = (sin32(yaw), cos32(yaw));
        let mut v = st.vel;
        v[0] += ms * v3 - v2 * i[1] * force;
        v[2] += ms * v2 + mf * v3;
        st.set_vel(v);
    }
}

/// Velocity change of an accepted jump; sprinting adds the table-based forward boost.
pub fn jump_impulse(mut v: Vec3, jump_height: f32, yaw: f32, sprinting: bool) -> Vec3 {
    v[1] = jump_height.max(v[1]);
    if sprinting {
        let f = yaw * 0.017453292;
        v[0] -= mc_sin(f) * 0.2;
        v[2] += mc_cos(f) * 0.2;
    }
    v
}

pub(crate) fn attempt_knockback(st: &mut PlayerState) {
    if let Some(k) = st.knockback {
        st.set_vel(k);
    }
}

pub(crate) fn effective_gravity(st: &PlayerState, v: Vec3) -> f32 {
    if st.slow_falling && v[1] < 0.0 { SLOW_FALLING_GRAVITY } else { st.gravity }
}

pub(crate) fn update_fall_distance(st: &mut PlayerState, old_y: f32) {
    let dy = st.pos[1] - old_y;
    if dy < 0.0 && !st.on_ground {
        st.fall_distance -= dy;
    } else if dy > 0.0 {
        st.fall_distance = 0.0;
    }
    if st.on_ground && st.fall_distance > 0.0 {
        st.fall_distance = 0.0;
    }
}

/// Whether the box lies over the supporting block's collision shape (`boxes`, block-local) in x and z (BDS;
/// honey's shape is inset 1/16); the look-back lookup can return a block the player has already walked off.
pub(crate) fn over_supporting_block(st: &PlayerState, boxes: &[Aabb]) -> bool {
    let (Some(p), bb) = (st.supporting_block, st.bounding_box()) else { return false };
    let (x, z) = (p[0] as f32, p[2] as f32);
    boxes.iter().any(|b| bb.max[0] > x + b.min[0] && bb.min[0] < x + b.max[0] && bb.max[2] > z + b.min[2] && bb.min[2] < z + b.max[2])
}

/// Slime and honey slow the walk by the vertical speed at the end of the tick (BDS step-on: landing ticks too,
/// and any downward speed; bedsim uses the pre-collision speed on level ticks only, |vy| < 0.1).
pub(crate) fn walk_on_block(st: &mut PlayerState, under: &BlockPhysics, final_vy: f32) {
    if !st.on_ground || st.sneaking {
        return;
    }
    let mut v = st.vel;
    if under.bounce == Bounce::Slime || under.honey {
        if final_vy < 0.1 {
            let d = 0.4 + final_vy.abs() * 0.2;
            v[0] *= d;
            v[2] *= d;
        }
    }
    st.set_vel(v);
}

fn land_on_block(st: &mut PlayerState, old: Vec3, under: &BlockPhysics, gravity: f32) {
    let mut v = st.vel;
    v[1] = if old[1] >= 0.0 || st.pressing_sneak {
        0.0
    } else {
        match under.bounce {
            // BDS `ComputeBlockRestitutionSystem`: no bounce below this downward speed, just above a standing
            // player's 0.0784 (a step down onto slime, a slow sink in water).
            Bounce::Slime if -old[1] < SLIME_MIN_BOUNCE_SPEED => 0.0,
            Bounce::Slime => {
                // BDS bounces with the speed at impact, accelerated by the medium's gravity over the distance
                // fallen this tick (st.vel holds the collided motion); bedsim uses the start-of-tick speed.
                let impact = (old[1] * old[1] + 2.0 * gravity * -st.vel[1]).sqrt();
                SLIME_BOUNCE_MULTIPLIER * -impact
            }
            Bounce::Bed => BED_BOUNCE_MULTIPLIER * old[1],
            Bounce::None => 0.0,
        }
    };
    st.set_vel(v);
}

pub(crate) fn set_post_collision_motion(st: &mut PlayerState, old: Vec3, old_on_ground: bool, under: &BlockPhysics, gravity: f32) {
    if !old_on_ground && st.collide_y {
        land_on_block(st, old, under, gravity);
    } else if st.collide_y {
        let mut v = st.vel;
        v[1] = 0.0;
        st.set_vel(v);
    }
    let mut v = st.vel;
    if st.collide_x {
        v[0] = 0.0;
    }
    if st.collide_z {
        v[2] = 0.0;
    }
    st.set_vel(v);
}

/// Elytra flight acceleration, as BDS's glide travel system computes it (2026-10-03, bdsre
/// `glideTravelVelocity`): the look vector from -pitch, its true horizontal length, gravity in BDS's form.
pub(crate) fn simulate_glide(st: &mut PlayerState) {
    if st.vel[1] > GLIDE_FALL_DISTANCE_VELOCITY_THRESHOLD {
        st.fall_distance = 1.0;
    }
    let radians = PI32 / 180.0;
    let (yaw, pitch) = (st.yaw * radians, st.pitch * radians);
    let yaw_cos = mc_cos(-yaw - PI32);
    let yaw_sin = mc_sin(-yaw - PI32);
    // The table index of cos(-p) can differ from cos(p)'s by one step.
    let look_pitch_cos = mc_cos(-pitch);
    let look = [yaw_sin * -look_pitch_cos, mc_sin(-pitch), yaw_cos * -look_pitch_cos];
    let pitch_cos = mc_cos(pitch);

    let mut v = st.vel;
    let vel_hz = (v[0] * v[0] + v[2] * v[2]).sqrt();
    let look_hz_sq = look[0] * look[0] + look[2] * look[2];
    let look_hz = look_hz_sq.sqrt();
    let sqr_pitch_cos = pitch_cos * pitch_cos;
    let gravity = if st.slow_falling { SLOW_FALLING_GRAVITY } else { st.gravity };
    v[1] -= (0.75 * sqr_pitch_cos + -1.0) * -gravity;
    if v[1] < 0.0 && look_hz_sq > 0.0 {
        let y_accel = v[1] * -0.1 * sqr_pitch_cos;
        v[1] += y_accel;
        v[0] += look[0] * y_accel / look_hz;
        v[2] += look[2] * y_accel / look_hz;
    }
    // BDS has no horizontal guard here; looking straight up would divide by zero.
    if pitch < 0.0 && look_hz_sq > 0.0 {
        let y_accel = vel_hz * -mc_sin(pitch) * 0.04;
        v[1] += y_accel * 3.2;
        v[0] -= look[0] * y_accel / look_hz;
        v[2] -= look[2] * y_accel / look_hz;
    }
    if look_hz_sq > 0.0 {
        v[0] += (look[0] / look_hz * vel_hz - v[0]) * 0.1;
        v[2] += (look[2] / look_hz * vel_hz - v[2]) * 0.1;
    }
    if st.glide_boost_ticks > 0 {
        for i in 0..3 {
            v[i] += (look[i] * 0.1) + (((look[i] * 1.5) - v[i]) * 0.5);
        }
    }
    v[0] *= 0.99;
    v[1] *= 0.98;
    v[2] *= 0.99;
    st.set_vel(v);
}

/// Sprint stall: the dominant requested axis barely moved.
pub(crate) fn sprint_movement_blocked(requested: Vec3, actual: Vec3) -> bool {
    const MIN_MOVEMENT: f32 = 0.00005;
    let (x, z) = (requested[0].abs(), requested[2].abs());
    (x > z && actual[0].abs() < MIN_MOVEMENT) || (z > x && actual[2].abs() < MIN_MOVEMENT)
}
