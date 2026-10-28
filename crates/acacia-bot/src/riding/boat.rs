//! Paddling a boat (physics bots): the boat is simulated from the bot's controls and reported in the
//! seated input, as the vanilla client predicts the boat it drives. Physics: `acacia_physics::boat_tick`.
//! The server's waves are partly random, so it corrects now and then (docs/research/riding-fishing-elytra.md
//! "Boat"): a correction resets the boat and the inputs sent since are replayed on top.

use std::collections::VecDeque;
use std::f64::consts::TAU;

use acacia_client::proto::packets::PlayerAuthInput;
use acacia_client::proto::types::{Vec2f, Vec3f};
use acacia_physics::{self as physics, BoatState, WorldView};

use super::keys::{keys, report_keys, report_paddles};
use crate::movement::VehicleCorrection;
use crate::world::PhysicsWorld;
use crate::Bot;

const BOATS: &[&str] = &["minecraft:boat", "minecraft:chest_boat"];
/// Inputs kept for replaying after a correction.
const HISTORY: usize = 40;
/// Ticks back a correction still tells which way the wave is going.
const RECENT: u64 = 8;
/// A correction off by more than this in x, z or yaw is horizontal (the M3 done bar counts those; design decision 5').
const HORIZONTAL: f32 = 1e-5;

/// The simulated boat of the current ride.
pub(crate) struct BoatSim {
    unique_id: i64,
    st: BoatState,
    /// Input tick, the keys held and the boat after it, oldest first.
    history: VecDeque<(u64, [f32; 2], BoatState)>,
    /// Tick and wave sine of the last correction.
    fixed: Option<(u64, f64)>,
    /// The ride's first input tick.
    first: Option<u64>,
}

impl BoatSim {
    /// Takes the server's boat after input `c.tick` and replays the inputs sent since. `Some(true)` when x, z or
    /// the yaw was off (more than `HORIZONTAL`), not just the height the waves move; `None` for one from before the
    /// ride's first input, the boat as boarded (the spawn packet's yaw is a 1.4° byte step).
    fn correct(&mut self, c: &VehicleCorrection, world: &impl WorldView) -> Option<bool> {
        let at = self.history.iter().position(|(tick, ..)| *tick == c.tick);
        let mut st = at.map_or(self.st, |i| self.history[i].2);
        let off = |a: f32, b: f32| (a - b).abs() > HORIZONTAL;
        let yaw_off = ((st.yaw - c.pitch_yaw[1] + 180.0).rem_euclid(360.0) - 180.0).abs() > HORIZONTAL;
        let boarding = self.first.is_none_or(|first| c.tick < first);
        let horizontal = !boarding && (off(st.pos[0], c.feet[0]) || off(st.pos[2], c.feet[2]) || yaw_off);
        if horizontal {
            tracing::debug!(tick = c.tick, ours = ?st.pos, vel = ?st.vel, yaw = st.yaw, server = ?c.feet, delta = ?c.delta, server_yaw = c.pitch_yaw[1], traced = at.is_some(), "horizontal boat correction");
        }
        (st.pos, st.vel, st.on_ground, st.yaw) = (c.feet, c.delta, c.on_ground, c.pitch_yaw[1]);
        st.yaw_vel = c.yaw_velocity.unwrap_or(st.yaw_vel);
        self.fix_wave(c.tick, &mut st);
        if let Some(i) = at {
            self.history[i].2 = st;
            for (_, held, after) in self.history.iter_mut().skip(i + 1) {
                physics::boat_tick(&mut st, *held, world);
                *after = st;
            }
        }
        self.st = st;
        (!boarding).then_some(horizontal)
    }

    /// Sets the wave to the phase the server's vertical motion shows. Two phases share a sine: the
    /// one that, stepped back, gives the sine of a recent correction, else the nearer to ours.
    fn fix_wave(&mut self, tick: u64, st: &mut BoatState) {
        let Some(phases) = physics::wave_phases(st) else {
            self.fixed = None;
            return;
        };
        let ours = st.wave;
        let apart = |phase: f64| (phase - ours).rem_euclid(TAU).min((ours - phase).rem_euclid(TAU));
        let step = physics::wave_step(st.vel);
        let error = |phase: f64| match self.fixed {
            Some((before, sine)) if (1..=RECENT).contains(&tick.wrapping_sub(before)) => {
                ((phase - step * (tick - before) as f64).sin() - sine).abs()
            }
            _ => apart(phase),
        };
        st.wave = if error(phases[0]) <= error(phases[1]) { phases[0] } else { phases[1] };
        self.fixed = Some((tick, phases[0].sin()));
    }
}

impl Bot {
    /// Corrections of boats this bot drove: (height only, horizontal), counted since it connected.
    pub fn boat_corrections(&self) -> (u32, u32) {
        let [height, horizontal] = self.ride.boat_corrections;
        (height, horizontal)
    }

    /// The simulated boat the bot drives, as of its last input (corrections arriving since not yet applied).
    pub fn boat_state(&self) -> Option<BoatState> {
        let vehicle = self.state.riding.vehicle.as_ref()?;
        self.ride.boat.as_ref().filter(|b| b.unique_id == vehicle.unique_id).map(|b| b.st)
    }

    /// Simulates the boat the bot drives for this tick and reports it in `input`: the boat's
    /// position, motion and yaw, the rider's keys and paddles. False when not driving a boat.
    pub(super) fn drive_boat(&mut self, input: &mut PlayerAuthInput) -> bool {
        let Some(vehicle) = self.state.riding.vehicle.as_ref().filter(|v| v.driver) else { return false };
        if !vehicle.kind.as_deref().is_some_and(|k| BOATS.contains(&k)) {
            return false;
        }
        let (Some(world), Some(movement)) = (&self.world, &mut self.movement) else { return false };
        let (Some(view), Some(registry)) = (world.view(), world.registry()) else { return false };
        let world = PhysicsWorld { view, registry };
        let (unique_id, runtime_id) = (vehicle.unique_id, vehicle.runtime_id);
        if self.ride.boat.as_ref().is_none_or(|b| b.unique_id != unique_id) {
            let Some(e) = runtime_id.and_then(|id| self.state.entities.get(id)) else { return false };
            let st = BoatState::new([e.position.x, e.position.y, e.position.z], e.yaw);
            self.ride.boat = Some(BoatSim { unique_id, st, history: VecDeque::with_capacity(HISTORY), fixed: None, first: None });
        }
        let Some(boat) = self.ride.boat.as_mut() else { return false };
        if let Some(c) = movement.vehicle_correction.take()
            && let Some(horizontal) = boat.correct(&c, &world)
        {
            self.ride.boat_corrections[usize::from(horizontal)] += 1;
        }
        let held = keys(&movement.controls);
        boat.first.get_or_insert(input.tick);
        physics::boat_tick(&mut boat.st, held, &world);
        if boat.history.len() == HISTORY {
            boat.history.pop_front();
        }
        boat.history.push_back((input.tick, held, boat.st));
        let st = &boat.st;
        tracing::trace!(tick = input.tick, pos = ?st.pos, delta = ?st.vel, yaw = st.yaw, wave = st.wave, ?held, "boat input");
        let [x, y, z] = st.pos;
        let [ox, oy, oz] = self.ride.report_offset.take().unwrap_or([0.0; 3]);
        input.position = Vec3f { x: x + ox, y: y + oy, z: z + oz };
        let [dx, dy, dz] = st.vel;
        input.delta = Vec3f { x: dx, y: dy, z: dz };
        input.vehicle_rotation = Some(Vec2f { x: 0.0, z: st.yaw });
        report_keys(input, held);
        report_paddles(input, held[0]);
        if let Some(id) = runtime_id {
            self.state.entities.set_pose(id, st.pos, st.yaw, 0.0);
        }
        true
    }
}
