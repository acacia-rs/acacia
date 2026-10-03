//! Moving items between slots with server-authoritative `ItemStackRequest`s: see the item methods
//! on [`crate::Bot`] (`move_item`, `swap_items`, `drop_item`, `quick_move`, ...).
//!
//! Slot naming follows vanilla (2026-10-02 capture): hotbar/inventory as `Hotbar`/`Inventory`,
//! except `HotbarAndInventory` for automatic moves (`request::auto_move`),
//! offhand as slot 1, chests and GUI menus as `LevelEntity` (`Container` here), barrels and shulker
//! boxes by their own types, furnaces and brewing stands per slot, workstation inputs by their UI
//! offset ([`ui`]).

mod actions;
pub(crate) mod craft;
mod plan;
mod quick;
mod request;
mod slot;
pub mod ui;

pub(crate) use plan::Plan;
pub(crate) use quick::{to_inventory_ops, to_inventory_ops_after};
pub(crate) use request::{beacon_payment, response_for, RequestIds};
pub(crate) use slot::is_own_screen;
#[cfg(test)]
pub(crate) use slot::{BlockKind, Screen};
pub use request::Op;
pub use slot::SlotRef;

#[cfg(test)]
mod tests;
