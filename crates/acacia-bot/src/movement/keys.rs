//! The only mapping from held [`Controls`] to the simulation's [`Input`]: the client rules that run on raw
//! key flags (double-tap sprint, sprint latch, flight double tap) and the rotation as sent.

use acacia_physics::{self as physics, Input, PlayerState, WorldView};

use super::{Controls, auth_input, flight};

/// Window for double-tapping forward to sprint.
const DOUBLE_TAP_TICKS: u32 = 7;

/// Key state carried between ticks.
#[derive(Debug, Clone, Default)]
pub(crate) struct KeyLatch {
    /// Ticks left in which pressing forward again starts a sprint (double tap).
    sprint_trigger: u32,
    prev_impulse: bool,
    /// The sprint outlives the (unheld) sprint key: a double tap started it, or a mouse client's key did.
    latched_sprint: bool,
    /// Ticks left of the jump double tap toggling the flight (`flight::fly_jump`).
    fly_taps: u8,
    /// Mouse input ([`crate::BotConfig::mouse_input`]): a released sprint key keeps the sprint.
    pub(crate) mouse_input: bool,
}

impl KeyLatch {
    /// This tick's input for `c` held by `st`; advances the latches.
    pub(crate) fn advance(&mut self, c: &Controls, st: &PlayerState, world: &(impl WorldView + ?Sized), using_item: bool, want_down: bool) -> Input {
        // Simulated with the rotation as sent, so a replay of the packet reproduces the tick exactly.
        let (yaw, pitch) = auth_input::wire_rotation(c.yaw, c.pitch);
        // Pressing forward again within DOUBLE_TAP_TICKS of the last press sprints: BDS runs this client
        // rule on the raw key flags, so the simulation must too. A press is the forward impulse reaching
        // full strength: sneaking scales it down on land (not in water, where it means sink), so releasing
        // sneak there with forward held counts. Presses while sneaking never count, and only presses on
        // the ground or in water do (vanilla client captures, strict BDS fuzz). A press with the sprint key
        // held starts no tap window (as in Java's `aiStep`).
        let forward = key(c.forward) > 0.0;
        let in_water = physics::touching_water(st, world);
        let sneaking = c.sneak || st.sneaking;
        let impulse = forward && !(sneaking && !in_water);
        self.sprint_trigger = self.sprint_trigger.saturating_sub(1);
        let mut double_tap = false;
        let grounded = st.on_ground || in_water;
        if impulse && !self.prev_impulse && !sneaking && grounded {
            if self.sprint_trigger > 0 {
                double_tap = true;
            } else if !c.sprint {
                self.sprint_trigger = DOUBLE_TAP_TICKS;
            }
        }
        self.prev_impulse = impulse;
        self.latched_sprint = if c.sprint { self.mouse_input } else { self.latched_sprint && st.sprinting || double_tap };
        Input {
            move_vector: keys(c.strafe, c.forward),
            yaw,
            pitch,
            jump: flight::fly_jump(&mut self.fly_taps, c, &st.flight),
            sneak: c.sneak,
            want_down,
            // A touch bot's key sprint ends with the key (strict BDS 1.26.52 fuzz, touch input: 94% of
            // key-released sprinting ticks were corrected). A mouse client's key sprint, like a double-tap
            // one, lasts while forward is held (vanilla Bedrock; accepted by strict BDS with mouse input).
            sprint: c.sprint || double_tap || (self.latched_sprint && st.sprinting && forward),
            using_item,
            glide: c.glide,
            ..Input::default()
        }
    }
}

/// The move vector of held keys: the client normalizes diagonals, so they move at the same 0.98 of full
/// speed as one key does (the simulation's own cap would let them reach 1.0).
fn keys(strafe: f32, forward: f32) -> [f32; 2] {
    let [s, f] = [key(strafe), key(forward)];
    if s != 0.0 && f != 0.0 { [s * std::f32::consts::FRAC_1_SQRT_2, f * std::f32::consts::FRAC_1_SQRT_2] } else { [s, f] }
}

/// A movement axis as a key press: -1, 0 or 1.
fn key(axis: f32) -> f32 {
    if axis > 0.0 { 1.0 } else if axis < 0.0 { -1.0 } else { 0.0 }
}
