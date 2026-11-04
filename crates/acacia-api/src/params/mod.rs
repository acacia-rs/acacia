//! Params and results of the methods in [`crate::METHODS`]. Every params type but a host method's has a `bot`
//! field (the `api!` table relies on it).

mod actions;
mod containers;
mod crafting;
mod host;
mod interact;
mod items;
mod moves;

pub use actions::*;
pub use containers::*;
pub use crafting::*;
pub use host::*;
pub use interact::*;
pub use items::*;
pub use moves::*;
