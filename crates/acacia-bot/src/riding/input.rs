//! The seated player's `PlayerAuthInput`, built by rewriting the standing-still input of the same tick.
//! Rules (vanilla capture 2026-10-02): docs/research/riding-fishing-elytra.md "PlayerAuthInput while riding".

use acacia_client::proto::packets::PlayerAuthInput;
use acacia_client::proto::types::{InputData as F, Vec2f, Vec3f};

use crate::movement::EYE_HEIGHT;
use crate::state::{GameState, Pose, Vehicle};

/// Vehicles the vanilla client predicts while it drives them ("Aka, Horse and Boat": Mojang
/// changelog_776; Geyser `processVehicleInput` adds camels, which are horses on Java, and not llamas).
const PREDICTED: &[&str] = &[
    "minecraft:boat",
    "minecraft:chest_boat",
    "minecraft:horse",
    "minecraft:donkey",
    "minecraft:mule",
    "minecraft:skeleton_horse",
    "minecraft:zombie_horse",
    "minecraft:camel",
];

/// Where the seated player is this tick.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Seat {
    /// The input position: the vehicle's own for a predicted vehicle, else the seat's.
    pub position: Vec3f,
    /// Unique id and rotation (pitch, yaw) of a vehicle the client predicts.
    pub predicted: Option<(i64, Vec2f)>,
}

impl Seat {
    pub(crate) fn of(state: &GameState) -> Option<Seat> {
        let vehicle = state.riding.vehicle.as_ref()?;
        let pose = vehicle_pose(state, vehicle);
        let predicts = vehicle.driver && vehicle.kind.as_deref().is_some_and(|k| PREDICTED.contains(&k));
        let seat = match (&pose, &state.riding.seat_offset) {
            (Some(p), Some(o)) => Vec3f { x: p.position.x + o.x, y: p.position.y + o.y, z: p.position.z + o.z },
            _ => state.player.eye_position(),
        };
        Some(match pose {
            // Without the vehicle's position a predicted move would drag the vehicle; report the seat instead.
            Some(p) if predicts => {
                Seat { position: p.position, predicted: Some((vehicle.unique_id, Vec2f { x: p.pitch, z: p.yaw })) }
            }
            _ => Seat { position: seat, predicted: None },
        })
    }

    /// Where the feet of a physics bot leaving this seat go: a predicted vehicle's position is the
    /// vehicle's own, not an eye position (subtracting the eye height put the bot a block underground).
    pub(crate) fn leaving_feet(&self) -> [f32; 3] {
        let p = &self.position;
        let eye = if self.predicted.is_some() { 0.0 } else { EYE_HEIGHT };
        [p.x, p.y - eye, p.z]
    }
}

/// The vehicle's latest pose: moves from the entity tracker when it runs, else its spawn.
pub(super) fn vehicle_pose(state: &GameState, vehicle: &Vehicle) -> Option<Pose> {
    let tracked = vehicle.runtime_id.and_then(|id| state.entities.get(id));
    match tracked {
        Some(e) => Some(Pose { position: e.position.clone(), pitch: e.pitch, yaw: e.yaw }),
        None => state.riding.spawn_pose(vehicle.unique_id).cloned(),
    }
}

/// Rewrites a standing-still input for a seated player. `first`: the first seated tick, where vanilla
/// still reports the standing delta outside a predicted vehicle (vehicle fields are already there).
pub(crate) fn apply_seat(input: &mut PlayerAuthInput, seat: &Seat, first: bool) {
    input.position = seat.position.clone();
    if !first || seat.predicted.is_some() {
        input.delta = Vec3f { x: 0.0, y: 0.0, z: 0.0 };
    }
    input.input_data.retain(|f| matches!(f, F::BlockBreakingDelayEnabled | F::HandledTeleport | F::VerticalCollision));
    if !input.input_data.contains(&F::VerticalCollision) {
        input.input_data.push(F::VerticalCollision);
    }
    (input.vehicle_rotation, input.predicted_vehicle) = (None, None);
    if let Some((unique_id, rotation)) = &seat.predicted {
        input.input_data.push(F::ClientPredictedVehicle);
        (input.vehicle_rotation, input.predicted_vehicle) = (Some(rotation.clone()), Some(*unique_id));
    }
}

/// Rewrites a standing input into vanilla's dismount tick: already standing at `eye`, no vehicle fields
/// and no sneak, and no VerticalCollision until the tick after.
pub(crate) fn apply_dismount(input: &mut PlayerAuthInput, eye: Vec3f) {
    input.position = eye;
    input.input_data.retain(|f| !matches!(f, F::VerticalCollision | F::ClientPredictedVehicle));
    (input.vehicle_rotation, input.predicted_vehicle) = (None, None);
}

#[cfg(test)]
mod tests;
