//! Creative flight as BDS 1.26.52 runs it (README "Flight" lists each rule's BDS function).

use crate::block_effects::apply_stuck_speed_multiplier;
use crate::constants::*;
use crate::math::{block_pos, sub};
use crate::motion::{attempt_knockback, move_relative, set_post_collision_motion};
use crate::sim::Sim;
use crate::state::PlayerState;
use crate::world::WorldView;

/// The flight abilities (`UpdateAbilities`) and state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight {
    pub may_fly: bool,
    /// Flying as toggled by this tick's input; the vertical input follows it at once.
    pub flying: bool,
    /// Flying as the travel sees it: BDS grants the toggle's ability request after the move, so the speed,
    /// drag and gravity follow it a tick late.
    pub travel: bool,
    pub fly_speed: f32,
    pub vertical_fly_speed: f32,
    /// Creative game mode, where a flight without input brakes harder.
    pub creative: bool,
    /// Ticks left in which a jump press toggles the flight (a double tap).
    pub trigger_ticks: i32,
}

impl Default for Flight {
    fn default() -> Self {
        Self {
            may_fly: false,
            flying: false,
            travel: false,
            fly_speed: DEFAULT_FLY_SPEED,
            vertical_fly_speed: 1.0,
            creative: false,
            trigger_ticks: 0,
        }
    }
}

/// BDS `VerticalFlySpeedControlSystem`: the vertical input before the move, while `flying`. Returns the friction
/// override of a tick without horizontal input.
pub(crate) fn control_vertical(st: &mut PlayerState) -> Option<f32> {
    if !st.flight.flying {
        return None;
    }
    let (up, down) = (st.pressing_jump, st.want_down);
    let mut v = st.vel;
    let mut idle_friction = None;
    if st.impulse[0].abs().max(st.impulse[1].abs()) < 0.01 {
        idle_friction = Some(if st.flight.creative { CREATIVE_IDLE_FLY_FRICTION } else { IDLE_FLY_FRICTION });
        if st.flight.creative && !up && !down {
            v[1] *= CREATIVE_IDLE_FLY_FRICTION;
        }
    }
    if up && down {
        v[1] = 0.0;
    } else {
        let mut accel = 0.0;
        if up {
            accel = FLY_UP_ACCELERATION;
        }
        if down {
            accel += FLY_DOWN_ACCELERATION;
        }
        v[1] = accel * st.flight.vertical_fly_speed + v[1];
    }
    st.set_vel(v);
    idle_friction
}

/// BDS post-move friction: an axis already within f32 epsilon of zero stops.
fn drag(v: f32, factor: f32) -> f32 {
    if v.abs() <= f32::EPSILON { 0.0 } else { factor * v }
}

impl<W: WorldView + ?Sized> Sim<'_, W> {
    pub(crate) fn fly(&self, st: &mut PlayerState) -> bool {
        let friction = if st.on_ground { self.w.block(block_pos(sub(st.pos, [0.0, 0.5, 0.0]))).friction } else { 1.0 };
        attempt_knockback(st);
        let idle_friction = control_vertical(st);
        let speed = st.flight.fly_speed * if st.sprinting { FLY_SPRINT_MULTIPLIER } else { 1.0 };
        move_relative(st, speed);
        let stuck = apply_stuck_speed_multiplier(st);
        if !self.sweep_loaded(st) {
            return false;
        }
        let mut old_vel = st.vel;
        let old_on_ground = st.on_ground;
        if !self.try_collisions(st) {
            return false;
        }
        st.fall_distance = 0.0;
        st.mov = st.vel;
        if stuck {
            st.set_vel([0.0; 3]);
            old_vel = [0.0; 3];
        }
        let under = self.w.block(st.supporting_block.unwrap_or_else(|| block_pos(sub(st.pos, [0.0, 0.2, 0.0]))));
        set_post_collision_motion(st, old_vel, old_on_ground, &under, st.gravity);
        let horizontal = match idle_friction {
            Some(o) => (friction * o) * DEFAULT_AIR_FRICTION,
            None => DEFAULT_AIR_FRICTION * friction,
        };
        let v = st.vel;
        st.set_vel([drag(v[0], horizontal), v[1] * FLY_VERTICAL_DRAG, drag(v[2], horizontal)]);
        self.apply_inside_block_effects(st);
        true
    }
}
