//! Rides in a trace: the [`Ride`] event, and replaying seated inputs, where a driven boat or horse goes through
//! the same simulation the bot drives with (`riding/boat.rs`, `riding/horse.rs`).

use std::io::{self, Read};

use acacia_physics::{Vec3, WorldView};

use super::read_u32;
use crate::movement::{Controls, VehicleCorrection};
use crate::riding::{keys, BoatSim, HorseSim};

/// What the simulation of a driven vehicle started from, and where the player resumed after a seat. Inputs
/// from a seat are recorded too.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ride {
    Boat { feet: Vec3, yaw: f32 },
    Horse { feet: Vec3, yaw: f32, speed: f32, jump_strength: f32 },
    Leave { feet: Vec3 },
    /// Seated on a vehicle the player does not simulate (yet): its inputs carry no player physics.
    Seat,
}

impl Ride {
    pub(super) fn tag_and_values(&self) -> (u8, Vec<f32>) {
        match *self {
            Ride::Boat { feet: [x, y, z], yaw } => (0, vec![x, y, z, yaw]),
            Ride::Horse { feet: [x, y, z], yaw, speed, jump_strength } => (1, vec![x, y, z, yaw, speed, jump_strength]),
            Ride::Leave { feet: [x, y, z] } => (2, vec![x, y, z]),
            Ride::Seat => (3, vec![]),
        }
    }

    pub(super) fn read(r: &mut impl Read) -> io::Result<Ride> {
        let mut tag = [0];
        r.read_exact(&mut tag)?;
        let mut f = |n: usize| (0..n).map(|_| read_u32(r).map(f32::from_bits)).collect::<io::Result<Vec<_>>>();
        Ok(match tag[0] {
            0 => {
                let v = f(4)?;
                Ride::Boat { feet: [v[0], v[1], v[2]], yaw: v[3] }
            }
            1 => {
                let v = f(6)?;
                Ride::Horse { feet: [v[0], v[1], v[2]], yaw: v[3], speed: v[4], jump_strength: v[5] }
            }
            2 => {
                let v = f(3)?;
                Ride::Leave { feet: [v[0], v[1], v[2]] }
            }
            3 => Ride::Seat,
            t => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("unknown ride event {t}"))),
        })
    }
}

/// A boat correction off by no more than this in x, z and yaw only moves the height: the server's waves are
/// partly random (riding/boat.rs), so it is no disagreement.
const BOAT_HORIZONTAL: f32 = 1e-5;

pub(super) enum Seat {
    Passenger,
    Boat(BoatSim),
    Horse(HorseSim),
}

/// A replayed vehicle after an input: wire position, velocity and yaw.
#[derive(Debug, Clone, Copy)]
pub(super) struct Driven {
    pub pos: Vec3,
    pub delta: Vec3,
    pub yaw: f32,
    boat: bool,
}

impl Seat {
    /// `None` for [`Ride::Leave`].
    pub(super) fn start(ride: &Ride) -> Option<Seat> {
        Some(match *ride {
            Ride::Seat => Seat::Passenger,
            Ride::Boat { feet, yaw } => Seat::Boat(BoatSim::new(0, feet, yaw)),
            Ride::Horse { feet, yaw, speed, jump_strength } => Seat::Horse(HorseSim::new(0, feet, yaw, speed, jump_strength)),
            Ride::Leave { .. } => return None,
        })
    }

    /// Steps the driven vehicle through input `tick`; `None` on a passenger's seat.
    pub(super) fn step(&mut self, tick: u64, c: &Controls, correction: Option<&VehicleCorrection>, world: &impl WorldView) -> Option<Driven> {
        match self {
            Seat::Passenger => None,
            Seat::Boat(b) => {
                b.step(tick, keys(c), correction, world);
                let st = b.state();
                Some(Driven { pos: st.pos, delta: st.vel, yaw: st.yaw, boat: true })
            }
            Seat::Horse(h) => {
                let out = h.step(c, correction, world);
                Some(Driven { pos: out.position, delta: out.delta, yaw: h.rotation()[1], boat: false })
            }
        }
    }
}

impl Driven {
    /// A correction of a boat's height alone (the waves).
    pub(super) fn wave_only(&self, c: &VehicleCorrection) -> bool {
        let yaw_gap = ((self.yaw - c.pitch_yaw[1] + 180.0).rem_euclid(360.0) - 180.0).abs();
        self.boat
            && (self.pos[0] - c.feet[0]).abs() <= BOAT_HORIZONTAL
            && (self.pos[2] - c.feet[2]).abs() <= BOAT_HORIZONTAL
            && yaw_gap <= BOAT_HORIZONTAL
    }
}
