//! Params and results of the methods in [`crate::METHODS`]. Every params type but a host method's has a `bot`
//! field (the `api!` table relies on it).

mod actions;
mod crafting;
mod host;
mod items;
mod moves;

pub use actions::*;
pub use crafting::*;
pub use host::*;
pub use items::*;
pub use moves::*;
