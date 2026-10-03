//! A player-steered horse, as BDS 1.26.52 moves one (bdsre 2026-10-03, docs/research/riding-fishing-elytra.md
//! "Horse"): the rider's input becomes the horse's, and the horse travels like a player on its own
//! box and attributes. Step height and the charged jump are not modelled yet.

use crate::input::Input;
use crate::sim::{TickOutput, tick};
use crate::state::PlayerState;
use crate::world::WorldView;
use crate::Vec3;

const HORSE_WIDTH: f32 = 1.4;
const HORSE_HEIGHT: f32 = 1.6;
/// BDS VariableMaxAutoStep with a controlling rider (0.5625 without, or on a block that prevents jumping).
const HORSE_STEP_HEIGHT: f32 = 1.0625;

/// The rider's controls for one tick.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RiderInput {
    /// Raw keys: [strafe, forward], each -1, 0 or 1.
    pub move_vector: [f32; 2],
    /// The rider's rotation in degrees.
    pub yaw: f32,
    pub pitch: f32,
}

/// A horse at `feet` facing `yaw`, moving at `speed` (its `minecraft:movement` attribute).
pub fn horse(feet: Vec3, yaw: f32, speed: f32) -> PlayerState {
    let mut st = PlayerState::new(feet);
    st.size = [HORSE_WIDTH, HORSE_HEIGHT, 1.0];
    st.ensure_pose_heights();
    st.yaw = yaw;
    st.set_movement_attribute(speed);
    st.fixed_air_speed = Some(speed * 0.1);
    st.step_height = HORSE_STEP_HEIGHT;
    st
}

/// One tick of a horse whose controlling rider gives `rider`. The yaw eases towards the rider's
/// (12.6%-35% of the gap per tick, more when the gap is small), strafing is halved and backing quartered.
pub fn horse_tick<W: WorldView + ?Sized>(st: &mut PlayerState, rider: &RiderInput, world: &W) -> TickOutput {
    let gap = wrap_degrees(rider.yaw - st.yaw);
    let rate = 0.7 * (0.18f32).max((45.0 - gap.abs().min(45.0)) / 90.0);
    let [strafe, forward] = rider.move_vector;
    let input = Input {
        move_vector: [strafe * 0.5, if forward > 0.0 { forward } else { forward * 0.25 }],
        yaw: st.yaw + gap * rate,
        pitch: rider.pitch * 0.5,
        ..Input::default()
    };
    tick(st, &input, world)
}

fn wrap_degrees(a: f32) -> f32 {
    let w = a.rem_euclid(360.0);
    if w >= 180.0 { w - 360.0 } else { w }
}
