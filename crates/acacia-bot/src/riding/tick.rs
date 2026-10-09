use acacia_client::proto::packets::PlayerAuthInput;
use acacia_client::proto::types::Vec3f;

use super::exit::Mount;
use super::input::{apply_dismount, apply_seat, Seat};
use super::keys::{keys, report_keys};
use crate::movement::{Idle, EYE_HEIGHT};
use crate::spawn::{self, SpawnPacket};
use crate::world::PhysicsWorld;
use crate::Bot;

impl Bot {
    /// Idle bots: turns this tick's standing input into a seated one while riding. A dismount steps
    /// off at the server's exit spot when the terrain around the vehicle is known, else in place.
    pub(crate) fn seat_idle_input(&mut self, input: &mut PlayerAuthInput) {
        let Some(seat) = Seat::of(&self.state) else {
            self.ride.seated = 0;
            return;
        };
        let exit = if self.ride.dismount { self.exit_feet() } else { None };
        self.seat_input(input, &seat, exit);
        if let Some(feet @ [x, y, z]) = exit.filter(|_| !self.state.riding.is_riding()) {
            tracing::debug!(?feet, "left the vehicle");
            self.state.player.position = Vec3f { x, y, z };
            if let Some((idle, _)) = self.idle.as_mut() {
                idle.stepped_to(feet);
            }
        }
    }

    /// Physics bots: while riding, sends a seated input instead of simulating (vehicle physics is not
    /// modelled) and returns true. On leaving the seat the simulation resumes where the server puts
    /// the player ([`Mount::exit_feet`]).
    pub(crate) fn ride_physics_tick(&mut self) -> bool {
        if self.movement.is_none() {
            return false;
        }
        let Some(seat) = Seat::of(&self.state) else {
            (self.ride.horse, self.ride.boat) = (None, None);
            if let Some(idle) = self.ride.idle.take() {
                let eye = self.state.player.eye_position();
                let feet = self.ride.leaving_feet.take().unwrap_or([eye.x, eye.y - EYE_HEIGHT, eye.z]);
                tracing::debug!(?feet, "left the vehicle");
                if let Some(movement) = self.movement.as_mut() {
                    movement.resync(idle.last_tick(), feet, [0.0; 3]);
                }
            }
            self.ride.seated = 0;
            return false;
        };
        let exit = self.exit_feet().unwrap_or_else(|| seat.leaving_feet());
        if self.ride.dismount {
            tracing::debug!(mount = ?Mount::of(&self.state), ?exit, "dismounting");
        }
        let Some(movement) = self.movement.as_mut() else { return false };
        let (yaw, pitch) = (movement.controls.yaw, movement.controls.pitch);
        let idle = self.ride.idle.get_or_insert_with(|| Idle::continuing(movement.input_tick()));
        if let Some(mut input) = idle.tick_facing(&self.state.player, yaw, pitch, true) {
            self.seat_input(&mut input, &seat, Some(exit));
            if self.state.riding.is_riding() && !self.drive_horse(&mut input) && !self.drive_boat(&mut input) {
                // The server moves the rest (pigs, minecarts) from the rider's keys.
                report_keys(&mut input, keys(&self.movement.as_ref().map(|m| m.controls).unwrap_or_default()));
            }
            self.add_queued_flags(&mut input);
            self.client.send(&input);
        }
        true
    }

    /// Where the server will put the player if it leaves the vehicle now, from the tracked terrain;
    /// `None` unless every block the search may test is known (idle bots keep only nearby sections).
    fn exit_feet(&self) -> Option<[f32; 3]> {
        let world = self.world.as_ref()?;
        let (view, registry) = (world.view()?, world.registry()?);
        let mount = Mount::of(&self.state)?;
        let [x, y, z] = mount.vehicle.map(|c| c.floor() as i32);
        let known = |dx, dy, dz| world.knows_block(x + dx, y + dy, z + dz);
        if ![-2, 2].iter().all(|&dx| [-2, 3].iter().all(|&dy| [-2, 2].iter().all(|&dz| known(dx, dy, dz)))) {
            return None;
        }
        mount.exit_feet(&PhysicsWorld { view, registry })
    }

    /// `exit`: feet of the player stepping off now (where a physics bot's simulation resumes); `None`
    /// steps off at the position the input already reports.
    fn seat_input(&mut self, input: &mut PlayerAuthInput, seat: &Seat, exit: Option<[f32; 3]>) {
        self.ride.leaving_feet = Some(exit.unwrap_or_else(|| seat.leaving_feet()));
        if std::mem::take(&mut self.ride.dismount) {
            let eye = match exit {
                Some([x, y, z]) => Vec3f { x, y: y + EYE_HEIGHT, z },
                None => input.position.clone(),
            };
            apply_dismount(input, eye.clone());
            self.leave_vehicle(eye);
            return;
        }
        // Vanilla clears aim assist and the crosshair target on the tick after the first seated one.
        if self.ride.seated == 1 {
            self.clear_aim_assist();
            spawn::send(&self.client, self.state.player.runtime_entity_id, SpawnPacket::MouseOverNothing);
        }
        apply_seat(input, seat, self.ride.seated == 0);
        self.ride.seated += 1;
    }
}
