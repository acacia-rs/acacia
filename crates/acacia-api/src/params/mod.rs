//! Params and results of the methods in [`crate::METHODS`]. Every params type but a host method's has a `bot`
//! field (the `api!` table relies on it).

mod actions;
mod host;
mod moves;

pub use actions::*;
pub use host::*;
pub use moves::*;
