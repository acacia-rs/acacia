//! Driving a horse (physics bots): the vehicle is simulated from the bot's controls and reported in the
//! seated input, as the vanilla client predicts the horse it rides. Physics: `acacia_physics::horse_tick`.

use acacia_client::proto::packets::PlayerAuthInput;
use acacia_client::proto::types::{InputData as F, Vec2f, Vec3f};
use acacia_physics::{self as physics, PlayerState, RiderInput};

use crate::world::PhysicsWorld;
use crate::Bot;

/// Horse-like vehicles the bot can steer.
const HORSES: &[&str] = &["minecraft:horse", "minecraft:donkey", "minecraft:mule"];

/// The simulated horse of the current ride.
pub(crate) struct HorseSim {
    unique_id: i64,
    st: PlayerState,
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
            self.ride.horse = Some(HorseSim { unique_id, st: physics::horse(feet, e.yaw, speed) });
        }
        let Some(horse) = self.ride.horse.as_mut() else { return false };
        if let Some(c) = movement.vehicle_correction.take() {
            horse.st.apply_correction(c.feet, c.delta, c.on_ground);
            // BDS also corrects a yaw gap alone; its easing then continues from the server's yaw.
            [horse.st.pitch, horse.st.yaw] = c.pitch_yaw;
        }
        let c = movement.controls;
        let keys = [axis(c.strafe), axis(c.forward)];
        let rider = RiderInput { move_vector: keys, yaw: c.yaw, pitch: c.pitch };
        let out = physics::horse_tick(&mut horse.st, &rider, &PhysicsWorld { view, registry });
        tracing::trace!(tick = input.tick, pos = ?out.position, delta = ?out.delta, yaw = horse.st.yaw, pitch = horse.st.pitch, on_ground = horse.st.on_ground, ?keys, "horse input");
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

/// The rider's movement keys, as vanilla reports them while riding (the pig capture: plain WASD).
fn report_keys(input: &mut PlayerAuthInput, [strafe, forward]: [f32; 2]) {
    for (on, flag) in [(forward > 0.0, F::Up), (forward < 0.0, F::Down), (strafe > 0.0, F::Left), (strafe < 0.0, F::Right)] {
        if on && !input.input_data.contains(&flag) {
            input.input_data.push(flag);
        }
    }
    input.move_vector = Vec2f { x: strafe, z: forward };
    input.raw_move_vector = Vec2f { x: strafe, z: forward };
}

/// A control axis as a key: any nonzero value counts as fully pressed.
fn axis(v: f32) -> f32 {
    if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 }
}
