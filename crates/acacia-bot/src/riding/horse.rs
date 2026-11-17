//! Driving a horse (physics bots): the vehicle is simulated from the bot's controls and reported in the
//! seated input, as the vanilla client predicts the horse it rides. Physics: `acacia_physics::horse_tick`.

use acacia_client::proto::packets::PlayerAuthInput;
use acacia_client::proto::types::{Vec2f, Vec3f};
use acacia_physics::{self as physics, HorseJump, PlayerState, RiderInput, TickOutput, WorldView};

use super::keys::{keys, report_jump, report_keys};
use crate::movement::{Controls, VehicleCorrection};
use crate::trace::Ride;
use crate::world::PhysicsWorld;
use crate::Bot;

/// Horse-like vehicles the bot can steer.
const HORSES: &[&str] = &["minecraft:horse", "minecraft:donkey", "minecraft:mule"];

/// The simulated horse of the current ride.
pub(crate) struct HorseSim {
    unique_id: i64,
    st: PlayerState,
    jump: HorseJump,
    /// The jump key was held on the last input.
    jump_held: bool,
}

impl HorseSim {
    pub(crate) fn new(unique_id: i64, feet: [f32; 3], yaw: f32, speed: f32, jump_strength: f32) -> Self {
        HorseSim { unique_id, st: physics::horse(feet, yaw, speed), jump: HorseJump::new(jump_strength), jump_held: false }
    }

    /// (pitch, yaw) after the last step.
    pub(crate) fn rotation(&self) -> [f32; 2] {
        [self.st.pitch, self.st.yaw]
    }

    /// Applies the server's `correction` if any, then simulates one input with the rider's controls `c`.
    pub(crate) fn step(&mut self, c: &Controls, correction: Option<&VehicleCorrection>, world: &impl WorldView) -> TickOutput {
        if let Some(c) = correction {
            self.st.apply_correction(c.feet, c.delta, c.on_ground);
            // BDS also corrects a yaw gap alone; its easing then continues from the server's yaw.
            [self.st.pitch, self.st.yaw] = c.pitch_yaw;
        }
        let rider = RiderInput { move_vector: keys(c), yaw: c.yaw, pitch: c.pitch, jump: c.jump };
        physics::horse_tick(&mut self.st, &mut self.jump, &rider, world)
    }
}

impl Bot {
    /// Simulates the horse the bot drives for this tick and reports it in `input`: the horse's
    /// position, motion and rotation, and the rider's movement keys. False when not driving a horse.
    pub(super) fn drive_horse(&mut self, input: &mut PlayerAuthInput) -> bool {
        let Some(vehicle) = self.state.riding.vehicle.as_ref().filter(|v| v.driver) else { return false };
        if !vehicle.kind.as_deref().is_some_and(|k| HORSES.contains(&k)) {
            return false;
        }
        let Some((world, movement)) = self.backend.physics_mut() else { return false };
        let (Some(view), Some(registry)) = (world.view(), world.registry()) else { return false };
        let (unique_id, runtime_id) = (vehicle.unique_id, vehicle.runtime_id);
        if self.ride.horse.as_ref().is_none_or(|h| h.unique_id != unique_id) {
            let Some(e) = runtime_id.and_then(|id| self.state.entities.get(id)) else { return false };
            let Some(&speed) = e.attributes.get("minecraft:movement") else { return false };
            let (feet, yaw) = ([e.position.x, e.position.y, e.position.z], e.yaw);
            let jump_strength = e.attributes.get("minecraft:horse.jump_strength").copied().unwrap_or_default();
            tracing::debug!(speed, jump_strength, "driving a horse");
            self.ride.horse = Some(HorseSim::new(unique_id, feet, yaw, speed, jump_strength));
            self.ride.started = Some(Ride::Horse { feet, yaw, speed, jump_strength });
        }
        let Some(horse) = self.ride.horse.as_mut() else { return false };
        let c = movement.controls;
        let out = horse.step(&c, movement.vehicle_correction.take().as_ref(), &PhysicsWorld { view, registry });
        let [pitch, yaw] = horse.rotation();
        tracing::trace!(tick = input.tick, pos = ?out.position, delta = ?out.delta, yaw, pitch, on_ground = horse.st.on_ground, jump = c.jump, "horse input");
        report_jump(input, c.jump, std::mem::replace(&mut horse.jump_held, c.jump));
        let [x, y, z] = out.position;
        let [ox, oy, oz] = self.ride.report_offset.take().unwrap_or([0.0; 3]);
        input.position = Vec3f { x: x + ox, y: y + oy, z: z + oz };
        let [dx, dy, dz] = out.delta;
        input.delta = Vec3f { x: dx, y: dy, z: dz };
        input.vehicle_rotation = Some(Vec2f { x: pitch, z: yaw });
        report_keys(input, keys(&c));
        if let Some(id) = runtime_id {
            self.state.entities.set_pose(id, out.position, yaw, pitch);
        }
        true
    }
}
