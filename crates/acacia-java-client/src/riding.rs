//! The local player's seat from `SetPassengers`, and the boat it drives, simulated as the vanilla client does
//! (`AbstractBoat.tick`, `LocalPlayer.rideTick` and `sendChanges`).
// TODO: other vehicles (horses, pigs, minecarts) seat the player but aren't driven; a passenger seat isn't placed.

use acacia_java_physics::{Boat, Keys};
use acacia_java_proto::entity_types::ENTITY_TYPES;
use acacia_java_proto::packets::{MovePlayerRot, MoveVehicle, PaddleBoat, PositionMoveRotation, VehiclePose};
use acacia_java_proto::{Wire, ids, read_varint};

use crate::frame::Error;
use crate::join::Client;

use ids::play::{clientbound as from, serverbound as to};

/// A position the server sets farther than this moves the driven vehicle (`handleMoveVehicle`).
const SNAP_DISTANCE: f64 = 1.0E-5_f32 as f64;

#[derive(Clone, Debug, PartialEq)]
pub struct Riding {
    pub vehicle: i32,
    /// In the controlling (first) seat.
    pub driver: bool,
    /// The boat the player drives, as simulated.
    pub boat: Option<Boat>,
    /// Last tick's keys: `rideTick` hands them to the boat after it ticked, so it moves by them a tick later.
    keys: Keys,
}

fn is_boat(name: &str) -> bool {
    name.ends_with("_boat") || name.ends_with("_raft")
}

impl Client {
    /// Riding packets, after the world applied them.
    pub(crate) fn apply_riding(&mut self, id: i32, body: &[u8]) -> Result<(), Error> {
        match id {
            from::SET_PASSENGERS | from::REMOVE_ENTITIES | from::RESPAWN | from::LOGIN => self.update_seat(),
            from::MOVE_VEHICLE => self.vehicle_moved(&mut &body[..])?,
            from::TELEPORT_ENTITY => self.vehicle_teleported(&mut &body[..])?,
            // `lerpMotion` sets any entity's motion, the driven boat's too.
            from::SET_ENTITY_MOTION => {
                let id = read_varint(&mut &body[..])?;
                let motion = self.world.entities.get(&id).map(|e| e.motion);
                if let Some((boat, motion)) = self.movement.riding.as_mut().and_then(|r| r.boat.as_mut()).zip(motion).filter(|_| self.world.driven == Some(id)) {
                    boat.delta = motion;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Takes the seat the passenger lists give the player. A new boat seat turns the player to the boat's yaw.
    fn update_seat(&mut self) {
        let me = self.world.player_id;
        let seat = self.world.entities.iter().find(|(_, e)| e.passengers.contains(&me)).map(|(&id, e)| (id, e.passengers[0] == me));
        let riding = &mut self.movement.riding;
        if seat == riding.as_ref().map(|r| (r.vehicle, r.driver)) {
            return;
        }
        let Some((vehicle, driver)) = seat else {
            *riding = None;
            self.world.driven = None;
            self.movement.player.delta = [0.0; 3];
            return;
        };
        let e = &self.world.entities[&vehicle];
        let name = usize::try_from(e.kind).ok().and_then(|k| ENTITY_TYPES.get(k)).copied().unwrap_or("");
        let same = riding.as_ref().filter(|r| r.vehicle == vehicle);
        if is_boat(name) && same.is_none() {
            self.movement.player.yaw = e.y_rot;
        }
        let kept = same.and_then(|r| r.boat.clone());
        let boat = (driver && is_boat(name)).then(|| {
            kept.unwrap_or_else(|| {
                let mut boat = Boat::new(&self.world, e.position, e.y_rot, name.ends_with("_raft"));
                boat.delta = e.motion;
                boat
            })
        });
        self.world.driven = boat.is_some().then_some(vehicle);
        *riding = Some(Riding { vehicle, driver, boat, keys: Keys::default() });
    }

    /// `handleMoveVehicle`: the server's pose for the driven vehicle, which the client answers with its own.
    fn vehicle_moved(&mut self, input: &mut &[u8]) -> Result<(), Error> {
        let pose = VehiclePose::read(input)?;
        let Some(boat) = self.movement.riding.as_mut().and_then(|r| r.boat.as_mut()) else { return Ok(()) };
        let target = [pose.x, pose.y, pose.z];
        let off: f64 = (0..3).map(|a| (target[a] - boat.pos[a]).powi(2)).sum::<f64>().sqrt();
        if off > SNAP_DISTANCE {
            let horizontal = [0, 2].iter().any(|&a| (target[a] - boat.pos[a]).abs() > SNAP_DISTANCE) || pose.y_rot % 360.0 != boat.yaw;
            self.movement.vehicle_corrections[usize::from(horizontal)] += 1;
            boat.pos = target;
            boat.yaw = pose.y_rot % 360.0;
            boat.pitch = pose.x_rot.clamp(-90.0, 90.0) % 360.0;
        }
        let echo = vehicle_move(boat);
        self.send(to::MOVE_VEHICLE, &echo)
    }

    /// A teleport of the driven boat snaps it; vanilla interpolates it over three ticks instead.
    fn vehicle_teleported(&mut self, input: &mut &[u8]) -> Result<(), Error> {
        let id = read_varint(input)?;
        let Some(boat) = self.movement.riding.as_mut().and_then(|r| r.boat.as_mut()).filter(|_| self.world.driven == Some(id)) else {
            return Ok(());
        };
        let c = PositionMoveRotation::read(input)?;
        let relatives = i32::read(input)?;
        let base = |bit: u32, v: f64| if relatives & (1 << bit) != 0 { v } else { 0.0 };
        boat.pos = [base(0, boat.pos[0]) + c.x, base(1, boat.pos[1]) + c.y, base(2, boat.pos[2]) + c.z];
        boat.delta = [base(5, boat.delta[0]) + c.delta_x, base(6, boat.delta[1]) + c.delta_y, base(7, boat.delta[2]) + c.delta_z];
        boat.yaw = base(3, boat.yaw as f64) as f32 + c.y_rot;
        boat.pitch = base(4, boat.pitch as f64) as f32 + c.x_rot;
        boat.on_ground = bool::read(input)?;
        Ok(())
    }

    /// A riding tick: the driven boat ticks with last tick's keys, the seat carries and turns the player, and the
    /// client reports both: `PaddleBoat`, then `sendChanges`' input, look and vehicle.
    pub(crate) fn ride_step(&mut self, keys: Keys) -> Result<(), Error> {
        let Some(riding) = self.movement.riding.as_mut() else { return Ok(()) };
        let player = &mut self.movement.player;
        let mut sent = None;
        if let Some(boat) = riding.boat.as_mut() {
            let [left, right] = boat.tick(&self.world, riding.keys);
            sent = Some((PaddleBoat { left, right }, boat.clone()));
            if let Some(e) = self.world.entities.get_mut(&riding.vehicle) {
                (e.position, e.y_rot, e.x_rot, e.on_ground) = (boat.pos, boat.yaw, boat.pitch, boat.on_ground);
            }
            player.pos = boat.rider_feet();
            player.yaw = boat.turn_rider(player.yaw);
        } else if let Some(e) = self.world.entities.get(&riding.vehicle) {
            player.pos = e.position;
        }
        riding.keys = keys;
        player.keys = keys;
        player.set_crouching(keys.shift);
        (player.delta, player.on_ground, player.horizontal_collision, player.sprinting) = ([0.0; 3], false, false, false);
        let (y_rot, x_rot) = (player.yaw, player.pitch);
        if let Some((paddles, _)) = &sent {
            self.send(to::PADDLE_BOAT, paddles)?;
        }
        self.send_keys()?;
        self.send(to::MOVE_PLAYER_ROT, &MovePlayerRot { y_rot, x_rot, flags: 0 })?;
        if let Some((_, boat)) = &sent {
            self.send(to::MOVE_VEHICLE, &vehicle_move(boat))?;
        }
        self.send_sprint()
    }
}

/// `ServerboundMoveVehiclePacket.fromEntity`.
fn vehicle_move(boat: &Boat) -> MoveVehicle {
    let [x, y, z] = boat.pos;
    MoveVehicle { pose: VehiclePose { x, y, z, y_rot: boat.yaw, x_rot: boat.pitch }, on_ground: boat.on_ground }
}
