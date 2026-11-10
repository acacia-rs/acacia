//! The vehicle the player drives, as the client simulates it: a boat or a mount.

use acacia_java_physics::{Boat, Mount};
use acacia_java_proto::packets::{MoveVehicle, VehiclePose};

#[derive(Clone, Debug, PartialEq)]
pub enum Driven {
    Boat(Boat),
    Mount(Box<Mount>),
}

impl Driven {
    pub fn boat(&self) -> Option<&Boat> {
        if let Driven::Boat(b) = self { Some(b) } else { None }
    }

    pub fn mount(&self) -> Option<&Mount> {
        if let Driven::Mount(m) = self { Some(m) } else { None }
    }

    pub(crate) fn mount_mut(&mut self) -> Option<&mut Mount> {
        if let Driven::Mount(m) = self { Some(m) } else { None }
    }

    /// Position, yaw and pitch.
    pub fn pose(&self) -> ([f64; 3], f32, f32) {
        match self {
            Driven::Boat(b) => (b.pos, b.yaw, b.pitch),
            Driven::Mount(m) => (m.body.pos, m.body.yaw, m.body.pitch),
        }
    }

    pub fn on_ground(&self) -> bool {
        match self {
            Driven::Boat(b) => b.on_ground,
            Driven::Mount(m) => m.body.on_ground,
        }
    }

    pub fn rider_feet(&self) -> [f64; 3] {
        match self {
            Driven::Boat(b) => b.rider_feet(),
            Driven::Mount(m) => m.rider_feet(),
        }
    }

    pub(crate) fn set_pose(&mut self, pos: [f64; 3], yaw: f32, pitch: f32) {
        match self {
            Driven::Boat(b) => (b.pos, b.yaw, b.pitch) = (pos, yaw, pitch),
            Driven::Mount(m) => (m.body.pos, m.body.yaw, m.body.pitch) = (pos, yaw, pitch),
        }
    }

    pub(crate) fn set_on_ground(&mut self, on_ground: bool) {
        match self {
            Driven::Boat(b) => b.on_ground = on_ground,
            Driven::Mount(m) => m.body.on_ground = on_ground,
        }
    }

    pub(crate) fn delta_mut(&mut self) -> &mut [f64; 3] {
        match self {
            Driven::Boat(b) => &mut b.delta,
            Driven::Mount(m) => &mut m.body.delta,
        }
    }

    pub(crate) fn interpolate_to(&mut self, pos: [f64; 3], yaw: f32, pitch: f32) {
        match self {
            Driven::Boat(b) => b.interpolate_to(pos, yaw, pitch),
            Driven::Mount(m) => m.interpolate_to(pos, yaw, pitch),
        }
    }

    /// `ServerboundMoveVehiclePacket.fromEntity`.
    pub(crate) fn vehicle_move(&self) -> MoveVehicle {
        let ([x, y, z], y_rot, x_rot) = self.pose();
        MoveVehicle { pose: VehiclePose { x, y, z, y_rot, x_rot }, on_ground: self.on_ground() }
    }
}
