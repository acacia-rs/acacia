//! Driving a horse (physics bots): the vehicle is simulated from the bot's controls and reported in the
//! seated input, as the vanilla client predicts the horse it rides. Physics: `acacia_physics::horse_tick`.

use acacia_client::proto::packets::PlayerAuthInput;
use acacia_client::proto::types::{Vec2f, Vec3f};
use acacia_physics::{self as physics, HorseJump, PlayerState, RiderInput};

use super::keys::{keys, report_jump, report_keys};
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

impl Bot {
    /// Simulates the horse the bot drives for this tick and reports it in `input`: the horse's
    /// position, motion and rotation, and the rider's movement keys. False when not driving a horse.
    pub(super) fn drive_horse(&mut self, input: &mut PlayerAuthInput) -> bool {
        let Some(vehicle) = self.state.riding.vehicle.as_ref().filter(|v| v.driver) else { return false };
        if !vehicle.kind.as_deref().is_some_and(|k| HORSES.contains(&k)) {
            return false;
        }
        let (Some(world), Some(movement)) = (&self.world, &mut self.movement) else { return false };
        let (Some(view), Some(registry)) = (world.view(), world.registry()) else { return false };
        let (unique_id, runtime_id) = (vehicle.unique_id, vehicle.runtime_id);
        if self.ride.horse.as_ref().is_none_or(|h| h.unique_id != unique_id) {
            let Some(e) = runtime_id.and_then(|id| self.state.entities.get(id)) else { return false };
            let Some(&speed) = e.attributes.get("minecraft:movement") else { return false };
            let feet = [e.position.x, e.position.y, e.position.z];
            let strength = e.attributes.get("minecraft:horse.jump_strength").copied().unwrap_or_default();
            tracing::debug!(speed, strength, "driving a horse");
            self.ride.horse = Some(HorseSim { unique_id, st: physics::horse(feet, e.yaw, speed), jump: HorseJump::new(strength), jump_held: false });
        }
        let Some(horse) = self.ride.horse.as_mut() else { return false };
        if let Some(c) = movement.vehicle_correction.take() {
            horse.st.apply_correction(c.feet, c.delta, c.on_ground);
            // BDS also corrects a yaw gap alone; its easing then continues from the server's yaw.
            [horse.st.pitch, horse.st.yaw] = c.pitch_yaw;
        }
        let c = movement.controls;
        let keys = keys(&c);
        let rider = RiderInput { move_vector: keys, yaw: c.yaw, pitch: c.pitch, jump: c.jump };
        let out = physics::horse_tick(&mut horse.st, &mut horse.jump, &rider, &PhysicsWorld { view, registry });
        tracing::trace!(tick = input.tick, pos = ?out.position, delta = ?out.delta, yaw = horse.st.yaw, pitch = horse.st.pitch, on_ground = horse.st.on_ground, ?keys, jump = c.jump, "horse input");
        report_jump(input, c.jump, std::mem::replace(&mut horse.jump_held, c.jump));
        let [x, y, z] = out.position;
        let [ox, oy, oz] = self.ride.report_offset.take().unwrap_or([0.0; 3]);
        input.position = Vec3f { x: x + ox, y: y + oy, z: z + oz };
        let [dx, dy, dz] = out.delta;
        input.delta = Vec3f { x: dx, y: dy, z: dz };
        input.vehicle_rotation = Some(Vec2f { x: horse.st.pitch, z: horse.st.yaw });
        report_keys(input, keys);
        if let Some(id) = runtime_id {
            self.state.entities.set_pose(id, out.position, horse.st.yaw, horse.st.pitch);
        }
        true
    }
}

