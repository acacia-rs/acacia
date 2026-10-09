//! A player-steered horse, as BDS 1.26.52 moves one (docs/research/riding-fishing-elytra.md "Horse"):
//! the rider's input becomes the horse's, and the horse travels like a player on its own box and
//! attributes, and leaps by its rider's charged jump ("Horse jump" there).

use crate::input::Input;
use crate::math::{mc_cos, mc_sin};
use crate::sim::{TickOutput, tick};
use crate::state::PlayerState;
use crate::world::WorldView;
use crate::Vec3;

const HORSE_WIDTH: f32 = 1.4;
const HORSE_HEIGHT: f32 = 1.6;
/// With a controlling rider (0.5625 without, or on a block that prevents jumping).
const HORSE_STEP_HEIGHT: f32 = 1.0625;
/// Ticks after a released jump before the key charges from zero again, counted up from here.
const HORSE_REST_TICKS: i32 = -10;
/// Forward speed a full jump adds.
const HORSE_LEAP: f32 = 0.4;

/// The rider's controls for one tick.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RiderInput {
    /// Raw keys: [strafe, forward], each -1, 0 or 1.
    pub move_vector: [f32; 2],
    /// The rider's rotation in degrees.
    pub yaw: f32,
    pub pitch: f32,
    /// The jump key is held.
    pub jump: bool,
}

/// A horse at `feet` facing `yaw`, moving at `speed` (its `minecraft:movement` attribute).
pub fn horse(feet: Vec3, yaw: f32, speed: f32) -> PlayerState {
    let mut st = PlayerState::new(feet);
    st.size = [HORSE_WIDTH, HORSE_HEIGHT, 1.0];
    st.ensure_pose_heights();
    st.yaw = yaw;
    st.set_movement_attribute(speed);
    // Strict BDS, natural horses (2026-10-09): the rider's keys reach the horse unscaled, on the ground and in the air.
    st.input_scale = 1.0;
    st.fixed_air_speed = Some(speed * 0.1);
    st.step_height = HORSE_STEP_HEIGHT;
    st
}

/// The charged jump of a ridden horse: the rider's jump key charges it, its release sets it off.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HorseJump {
    /// The horse's `minecraft:horse.jump_strength`.
    pub strength: f32,
    /// Ticks the key has charged for; negative while the horse rests after a release.
    ticks: i32,
    scale: f32,
    held: bool,
    /// Power of a released jump still to leave the ground, 0.4 to 1.
    pending: f32,
}

impl HorseJump {
    pub fn new(strength: f32) -> Self {
        HorseJump { strength, ..HorseJump::default() }
    }

    /// One tick of the rider's jump key: 0.1 of charge per held tick up to full on the 11th, then
    /// falling towards 0.8. The first held tick charges nothing.
    fn charge(&mut self, jump: bool) {
        if self.ticks < 0 {
            self.ticks += 1;
            if self.ticks == 0 {
                self.scale = 0.0;
            }
        }
        if !std::mem::replace(&mut self.held, jump) {
            return;
        }
        if jump {
            self.scale = if self.ticks < 9 { (self.ticks + 1) as f32 * 0.1 } else { (2.0 / (self.ticks - 8) as f32) * 0.1 + 0.8 };
            self.ticks += 1;
            return;
        }
        self.ticks = HORSE_REST_TICKS;
        let amount = (self.scale * 100.0) as i32;
        // A key pressed again while the horse rests charges below zero and jumps nothing.
        if amount >= 0 {
            self.pending = if amount >= 90 { 1.0 } else { amount as f32 * 0.4 / 90.0 + 0.4 };
        }
    }
}

/// One tick of a horse whose controlling rider gives `rider`. The yaw eases towards the rider's
/// (12.6%-35% of the gap per tick, more when the gap is small), strafing is halved and backing quartered.
/// A released jump leaves the ground with the charged share of the horse's strength, pushed forward
/// when the horse is ridden forward.
pub fn horse_tick<W: WorldView + ?Sized>(st: &mut PlayerState, jump: &mut HorseJump, rider: &RiderInput, world: &W) -> TickOutput {
    let gap = wrap_degrees(rider.yaw - st.yaw);
    let rate = 0.7 * (0.18f32).max((45.0 - gap.abs().min(45.0)) / 90.0);
    // Two keys held share the vector's length before strafing is halved (W+D, strict BDS 2026-10-09).
    let length = rider.move_vector[0].hypot(rider.move_vector[1]).max(1.0);
    let [strafe, forward] = rider.move_vector.map(|key| key / length);
    let yaw = st.yaw + gap * rate;
    jump.charge(rider.jump);
    let leaps = jump.pending > 0.0 && st.on_ground;
    if leaps {
        st.jump_strength = jump.strength * jump.pending;
        if forward > 0.0 {
            let heading = yaw * 0.017453292;
            st.vel[0] -= mc_sin(heading) * HORSE_LEAP * jump.pending;
            st.vel[2] += mc_cos(heading) * HORSE_LEAP * jump.pending;
        }
    }
    let input = Input {
        move_vector: [strafe * 0.5, if forward > 0.0 { forward } else { forward * 0.25 }],
        yaw,
        pitch: rider.pitch * 0.5,
        jump: leaps,
        ..Input::default()
    };
    let out = tick(st, &input, world);
    // A jump released in the air is lost on landing.
    if st.on_ground || leaps {
        jump.pending = 0.0;
    }
    out
}

fn wrap_degrees(a: f32) -> f32 {
    let w = a.rem_euclid(360.0);
    if w >= 180.0 { w - 360.0 } else { w }
}
