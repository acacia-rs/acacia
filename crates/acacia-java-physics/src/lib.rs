//! Java Edition player and boat movement. Floats and doubles follow the vanilla client's types exactly, because the
//! comparison against client traces is bit for bit.

mod aabb;
pub mod attributes;
mod block_factors;
mod block_path;
mod boat;
mod boat_inside;
mod boat_status;
mod blocks;
mod collide;
mod fluid;
mod input;
mod inside;
mod inside_shape;
mod lerp;
mod mount;
mod mount_body;
mod mount_ridden;
mod mount_seat;
mod moving;
mod mth;
mod open_map;
mod riptide;
mod shove;
mod player;
mod support;
mod swim;
mod teleport;
mod tick;
mod travel;

pub use aabb::Aabb;
pub use boat::{clamp_rider, ride_height, Boat, BoatStatus};
pub use collide::Blocks;
pub use fluid::fluid_of;
pub use input::{Keys, Pose};
pub use mount::{Boost, Mount, MountKind, Rider};
pub use mount_seat::{feet_at, seat_feet, turned, SeatAnim};
pub use shove::shove;
pub use mth::sin;
pub use player::{Effects, ItemUse, Player};
pub use riptide::Nearby;
pub use teleport::Placement;
