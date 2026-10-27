//! The client-side menus' slot layouts (`AbstractContainerMenu` subclasses' constructors): where
//! each slot sits in the window, in menu order.

use super::Slot;

/// Slots `first..` in a `cols` × `rows` grid of 18-pixel cells from `(left, top)`.
pub fn grid(slots: &mut Vec<Slot>, cols: i32, rows: i32, left: i32, top: i32) {
    for y in 0..rows {
        for x in 0..cols {
            slots.push(Slot::new(slots.len(), left + x * 18, top + y * 18));
        }
    }
}

/// `addStandardInventorySlots`: the three inventory rows, then the hotbar 58 below their top.
pub fn standard_inventory(slots: &mut Vec<Slot>, left: i32, top: i32) {
    grid(slots, 9, 3, left, top);
    grid(slots, 9, 1, left, top + 58);
}

/// `ChestMenu` of `rows` rows.
pub fn chest(rows: i32) -> Vec<Slot> {
    let mut slots = Vec::new();
    grid(&mut slots, 9, rows, 8, 18);
    standard_inventory(&mut slots, 8, 18 + rows * 18 + 13);
    slots
}

/// `ShulkerBoxMenu`.
pub fn shulker() -> Vec<Slot> {
    let mut slots = Vec::new();
    grid(&mut slots, 9, 3, 8, 18);
    standard_inventory(&mut slots, 8, 84);
    slots
}

/// `DispenserMenu` (dispenser and dropper).
pub fn dispenser() -> Vec<Slot> {
    let mut slots = Vec::new();
    grid(&mut slots, 3, 3, 62, 17);
    standard_inventory(&mut slots, 8, 84);
    slots
}

/// `HopperMenu`.
pub fn hopper() -> Vec<Slot> {
    let mut slots = Vec::new();
    grid(&mut slots, 5, 1, 44, 20);
    standard_inventory(&mut slots, 8, 51);
    slots
}
