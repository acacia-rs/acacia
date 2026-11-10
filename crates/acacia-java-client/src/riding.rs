//! The local player's seat from `SetPassengers`, and the vehicle it drives, simulated as the vanilla client does: the
//! vehicle ticks first (`AbstractBoat.tick`, or a mount's `travelRidden`), then `LocalPlayer.rideTick` and
//! `sendChanges`. A vehicle the client doesn't control (a minecart, an unsaddled horse) only carries the player.

use acacia_java_physics::{Boat, Keys, MountKind, Placement, Rider};
use acacia_java_proto::packets::{COMMAND_START_RIDING_JUMP, MovePlayerRot, PaddleBoat, PositionMoveRotation, VehiclePose};
use acacia_java_proto::{Wire, ids, read_varint};

use crate::driven::Driven;
use crate::frame::Error;
use crate::join::Client;
use crate::mount_data::{Synced, short_name};
use crate::mount_drive::{JumpBar, apply_attributes, apply_synced};
use crate::movement::placement;
use crate::world::Mover;

use ids::play::{clientbound as from, serverbound as to};

/// A position the server sets farther than this moves the driven vehicle (`handleMoveVehicle`).
const SNAP_DISTANCE: f64 = 1.0E-5_f32 as f64;

#[derive(Clone, Debug, PartialEq)]
pub struct Riding {
    pub vehicle: i32,
    /// In the controlling (first) seat.
    pub driver: bool,
    /// The vehicle the player controls, as simulated.
    pub driven: Option<Driven>,
    /// Last tick's keys and rider: the vehicle ticks before the player, so it moves by them a tick later.
    keys: Keys,
    rider: Rider,
    pub(crate) jump_bar: JumpBar,
}

fn is_boat(name: &str) -> bool {
    name.ends_with("_boat") || name.ends_with("_raft")
}

/// `AbstractHorse.addPassenger` turns the rider's view to the horse's.
fn snaps_view(name: &str) -> bool {
    matches!(name, "horse" | "donkey" | "mule" | "skeleton_horse" | "zombie_horse" | "llama" | "trader_llama" | "camel" | "camel_husk")
}

impl Client {
    /// Riding packets, after the world applied them.
    pub(crate) fn apply_riding(&mut self, id: i32, body: &[u8]) -> Result<(), Error> {
        let input = &mut &body[..];
        match id {
            from::SET_PASSENGERS | from::REMOVE_ENTITIES | from::RESPAWN | from::LOGIN => self.update_seat(),
            from::MOVE_VEHICLE => self.vehicle_moved(input)?,
            from::TELEPORT_ENTITY => self.vehicle_teleported(input)?,
            // `lerpMotion` sets any entity's motion, the driven vehicle's too.
            from::SET_ENTITY_MOTION => {
                let id = read_varint(input)?;
                let motion = self.world.entities.get(&id).map(|e| e.motion);
                if let (Some(driven), Some(motion)) = (self.driven_mut(id), motion) {
                    *driven.delta_mut() = motion;
                }
            }
            from::ENTITY_EVENT => {
                let (id, event) = (i32::read(input)?, i8::read(input)?);
                if let (Some(Driven::Boat(boat)), 71 | 72) = (self.driven_mut(id), event) {
                    boat.bubble_column_effect(event == 71);
                }
            }
            from::SET_ENTITY_DATA | from::UPDATE_ATTRIBUTES => {
                let id2 = read_varint(input)?;
                let Some(e) = self.world.entities.get(&id2) else { return Ok(()) };
                let (kind, data) = (e.kind, e.mount.clone());
                let synced = if id == from::SET_ENTITY_DATA { Some(Synced::read(kind, input)?) } else { None };
                if let Some(Driven::Mount(mount)) = self.driven_mut(id2) {
                    match synced {
                        Some(synced) => apply_synced(mount, synced),
                        None => apply_attributes(mount, &data),
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn driven_mut(&mut self, id: i32) -> Option<&mut Driven> {
        self.movement.riding.as_mut().and_then(|r| r.driven.as_mut()).filter(|_| self.world.driven == Some(id))
    }

    /// Takes the seat the passenger lists give the player. A new boat or horse seat turns the player to the vehicle.
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
        let name = short_name(e.kind);
        let same = riding.as_ref().filter(|r| r.vehicle == vehicle);
        if same.is_none() && is_boat(name) {
            self.movement.player.yaw = e.y_rot;
        } else if same.is_none() && snaps_view(name) {
            (self.movement.player.yaw, self.movement.player.pitch) = (e.y_rot, e.x_rot);
        }
        let kept = same.and_then(|r| r.driven.clone()).filter(|_| driver);
        let driven = kept.or_else(|| {
            (driver && is_boat(name)).then(|| {
                let mut boat = Boat::new(&self.world, e.position, e.y_rot, name.ends_with("_raft"));
                boat.delta = e.motion;
                Driven::Boat(boat)
            })
        });
        self.world.driven = driven.is_some().then_some(vehicle);
        *riding = Some(Riding { vehicle, driver, driven, keys: Keys::default(), rider: Rider::default(), jump_bar: JumpBar::default() });
    }

    /// Starts or stops simulating the mount under the player as control comes and goes (a pig's rider switching away
    /// from the carrot on a stick, a horse losing its saddle).
    fn update_mount(&mut self) {
        let Some(riding) = self.movement.riding.as_mut().filter(|r| r.driver) else { return };
        if matches!(riding.driven, Some(Driven::Boat(_))) {
            return;
        }
        let vehicle = riding.vehicle;
        let kept = riding.driven.take().and_then(|d| d.mount().cloned());
        let driven = self.controlled_mount(vehicle, kept).map(|m| Driven::Mount(Box::new(m)));
        self.world.driven = driven.is_some().then_some(vehicle);
        if let Some(riding) = self.movement.riding.as_mut() {
            riding.driven = driven;
        }
    }

    /// `handleMoveVehicle`: the server's pose for the driven vehicle, which the client answers with its own.
    fn vehicle_moved(&mut self, input: &mut &[u8]) -> Result<(), Error> {
        let pose = VehiclePose::read(input)?;
        let Some(driven) = self.movement.riding.as_mut().and_then(|r| r.driven.as_mut()) else { return Ok(()) };
        let (pos, yaw, _) = driven.pose();
        let target = [pose.x, pose.y, pose.z];
        let off: f64 = (0..3).map(|a| (target[a] - pos[a]).powi(2)).sum::<f64>().sqrt();
        if off > SNAP_DISTANCE {
            let horizontal = [0, 2].iter().any(|&a| (target[a] - pos[a]).abs() > SNAP_DISTANCE) || pose.y_rot % 360.0 != yaw;
            self.movement.vehicle_corrections[usize::from(horizontal)] += 1;
            driven.set_pose(target, pose.y_rot % 360.0, pose.x_rot.clamp(-90.0, 90.0) % 360.0);
        }
        let echo = driven.vehicle_move();
        self.send(to::MOVE_VEHICLE, &echo)
    }

    /// `handleTeleportEntity` on the driven vehicle: interpolated over three ticks; beyond 64 blocks it snaps,
    /// carries the rider and is reported at once.
    fn vehicle_teleported(&mut self, input: &mut &[u8]) -> Result<(), Error> {
        let id = read_varint(input)?;
        let Some(driven) = self.driven_mut(id) else { return Ok(()) };
        let change = placement(&PositionMoveRotation::read(input)?);
        let (now, y_rot, x_rot) = driven.pose();
        let source = Placement { pos: now, delta: *driven.delta_mut(), y_rot, x_rot };
        let Placement { pos, delta, y_rot, x_rot } = source.teleported(&change, i32::read(input)?);
        *driven.delta_mut() = delta;
        let snap = (0..3).map(|a| (pos[a] - now[a]).powi(2)).sum::<f64>() > 4096.0;
        if snap {
            driven.set_pose(pos, y_rot, x_rot);
        } else {
            driven.interpolate_to(pos, y_rot, x_rot);
        }
        driven.set_on_ground(bool::read(input)?);
        if snap {
            let (echo, feet) = (driven.vehicle_move(), driven.rider_feet());
            self.movement.player.pos = feet;
            self.send(to::MOVE_VEHICLE, &echo)?;
        }
        Ok(())
    }

    /// A riding tick: the driven vehicle ticks with last tick's input, the seat carries the player, the player's
    /// `aiStep` reads the keys and charges a mount's jump, and the client reports: `PaddleBoat` or the riding jump,
    /// then `sendChanges`' input, look and vehicle.
    pub(crate) fn ride_step(&mut self, keys: Keys) -> Result<(), Error> {
        self.update_mount();
        let Some(vehicle) = self.movement.riding.as_ref().map(|r| r.vehicle) else { return Ok(()) };
        let carried = self.passenger_feet(vehicle);
        let passengers = self.world.entities.get(&vehicle).map_or(&[][..], |e| &e.passengers);
        let solid = self.world.collision_boxes(Mover::Vehicle, &[&[vehicle][..], passengers].concat());
        let (Some(riding), player) = (self.movement.riding.as_mut(), &mut self.movement.player) else { return Ok(()) };
        let mut paddles = None;
        match riding.driven.as_mut() {
            Some(Driven::Boat(boat)) => {
                boat.solid_entities = solid;
                let [left, right] = boat.tick(&self.world, riding.keys);
                paddles = Some(PaddleBoat { left, right });
            }
            Some(Driven::Mount(mount)) => {
                mount.body.solid_entities = solid;
                mount.game_time = self.world.game_time;
                mount.tick(&self.world, riding.rider);
            }
            None => {}
        }
        if let Some(driven) = &riding.driven {
            if let Some(e) = self.world.entities.get_mut(&vehicle) {
                ((e.position, e.y_rot, e.x_rot), e.on_ground) = (driven.pose(), driven.on_ground());
            }
            player.pos = driven.rider_feet();
            if let Some(boat) = driven.boat() {
                player.yaw = boat.turn_rider(player.yaw);
            }
        } else if let Some(feet) = carried {
            player.pos = feet;
        }
        let was_jumping = std::mem::replace(&mut riding.keys, keys).jump;
        let camel = riding.driven.as_ref().and_then(Driven::mount).is_some_and(|m| m.kind == MountKind::Camel);
        // TODO: vanilla's sprint rules (double tap, collisions) aren't run for a camel's rider; the key alone sprints.
        player.sprinting = camel && keys.sprint && keys.forward && !keys.backward;
        riding.rider = player.rider(keys);
        player.set_crouching(keys.shift);
        (player.delta, player.on_ground, player.horizontal_collision) = ([0.0; 3], false, false);
        let (y_rot, x_rot) = (player.yaw, player.pitch);
        let moved = riding.driven.as_ref().map(Driven::vehicle_move);
        let charge = self.charge_jump(was_jumping, keys.jump);
        if let Some(paddles) = &paddles {
            self.send(to::PADDLE_BOAT, paddles)?;
        }
        if let Some(charge) = charge {
            self.send_command_with(COMMAND_START_RIDING_JUMP, charge)?;
        }
        self.send_keys()?;
        self.send(to::MOVE_PLAYER_ROT, &MovePlayerRot { y_rot, x_rot, flags: 0 })?;
        if let Some(moved) = &moved {
            self.send(to::MOVE_VEHICLE, moved)?;
        }
        self.send_sprint()
    }
}
