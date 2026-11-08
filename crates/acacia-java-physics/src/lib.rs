//! Java Edition player movement. Floats and doubles follow the vanilla client's types exactly, because the
//! comparison against client traces is bit for bit.

mod aabb;
mod block_factors;
mod collide;
mod mth;
mod player;
mod travel;

pub use aabb::Aabb;
pub use collide::Blocks;
pub use player::{Keys, Player};
