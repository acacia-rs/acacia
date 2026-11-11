//! Java Edition player and boat movement. Floats and doubles follow the vanilla client's types exactly, because the
//! comparison against client traces is bit for bit.

mod aabb;
mod block_factors;
mod block_path;
mod boat;
mod boat_inside;
mod boat_status;
mod collide;
mod fluid;
mod input;
mod inside;
mod lerp;
mod mount;
mod mount_body;
mod mount_ridden;
mod moving;
mod mth;
mod player;
mod support;
mod swim;
mod teleport;
mod tick;
mod travel;

pub use aabb::Aabb;
pub use boat::{Boat, BoatStatus};
pub use collide::Blocks;
pub use input::{Keys, Pose};
pub use mount::{seat_feet, Boost, Mount, MountKind, Rider};
pub use player::{Effects, ItemUse, Player};
pub use teleport::Placement;
