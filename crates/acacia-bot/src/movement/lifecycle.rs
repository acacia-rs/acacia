//! Starting, stopping and re-aligning the simulation: at spawn, across a dimension change (docs/DESIGN.md
//! "Dimension travel") and in replays.

use acacia_physics::{PlayerState, Vec3};

use super::rewind::History;
use super::Movement;

impl Movement {
    /// Starts simulating from the spawn position (feet).
    pub fn start(&mut self, feet: Vec3, yaw: f32, pitch: f32) {
        let mut st = PlayerState::new(feet);
        st.effects = self.effects;
        if let Some(value) = self.carried_attribute.take() {
            st.set_movement_attribute(value);
            // BDS carries the standing state across a dimension change: on the ground with one tick of
            // gravity, so a player put in mid-air falls 0.0784 on the first input (live, 1.26.52).
            st.apply_correction(feet, super::idle::STANDING_DELTA, true);
        }
        self.physics = Some(st);
        (self.controls.yaw, self.controls.pitch) = (yaw, pitch);
    }

    /// Stops simulating until the next [`Movement::start`]: the position, the rewind history and the server
    /// moves still queued belong to the dimension left. The input tick count goes on.
    pub(super) fn leave_dimension(&mut self) {
        if let Some(st) = self.physics.take() {
            self.carried_attribute = Some(st.movement_attribute);
        }
        self.history = History::default();
        self.spawn_wait = 0;
        (self.ack_teleport, self.current_on_landing) = (false, false);
        self.vehicle_correction = None;
        // The server's flying flag is adopted again on the first tick there.
        self.server_flying = None;
        (self.prev_jump, self.prev_impulse, self.tapped_sprint) = (false, false, false);
        (self.sprint_trigger, self.fly_taps) = (0, 0);
    }

    /// Replaces the simulated position and velocity as of the end of input `tick`.
    pub(crate) fn resync(&mut self, tick: u64, feet: Vec3, delta: Vec3) {
        if let Some(st) = &mut self.physics {
            st.apply_correction(feet, delta, st.on_ground);
            self.tick = tick;
        }
    }

    /// Numbers the next simulated tick `tick`: a real client's tick counter can skip (replay only).
    pub(crate) fn align_tick(&mut self, tick: u64) {
        self.tick = tick.saturating_sub(1);
    }

    /// Tick of the last `PlayerAuthInput` built.
    pub(crate) fn input_tick(&self) -> u64 {
        self.tick
    }

    pub fn is_started(&self) -> bool {
        self.physics.is_some()
    }
}
