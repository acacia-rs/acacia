//! Bedrock player movement simulation, ported from oomph-ac/bedsim (see README.md).
//!
//! Network-free: the world is reached through [`WorldView`]; one [`tick`] is one 50 ms client tick.

// Per-axis index loops mirror bedsim's Go source line for line.
#![allow(clippy::needless_range_loop)]

mod aabb;
mod block_effects;
mod clip;
mod collide;
pub mod constants;
mod input;
mod liquid;
mod liquid_travel;
mod math;
mod motion;
mod movement;
mod pose;
mod push_out;
mod sim;
mod state;
pub mod test_world;
mod vehicle;
mod world;

pub use aabb::{Aabb, INTERSECT_EPSILON};
pub use clip::clip_collide;
pub use input::Input;
pub use math::{BlockPos, Vec3, mc_cos, mc_sin};
pub use motion::jump_impulse;
pub use sim::{Outcome, TickOutput, apply_current, tick, touching_water};
pub use state::{Effects, Equipment, PlayerState};
pub use vehicle::{RiderInput, horse, horse_tick};
pub use world::{BlockPhysics, Bounce, BubbleColumn, InsideMovement, Liquid, LiquidKind, Traversal, WorldView};
