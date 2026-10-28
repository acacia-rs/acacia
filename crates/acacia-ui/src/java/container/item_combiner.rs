//! What the station screens share: `ItemCombinerScreen`'s window (the texture, then the subclass's
//! error icon), the stations' slot layouts, and what the client reads of a station's state
//! besides its slots.

use super::{Base, Slot, blit, menus, transparent_background};
use crate::java::gui::Gui;

/// The player state and server-synced data a station screen reads.
#[derive(Clone, Debug, Default)]
pub struct Station {
    /// `player.experienceLevel`.
    pub xp: i32,
    /// `player.hasInfiniteMaterials()`.
    pub creative: bool,
    /// The menu's data slots, in `addDataSlot` order.
    pub data: Vec<i32>,
}

impl Station {
    pub fn data(&self, i: usize) -> i32 {
        self.data.get(i).copied().unwrap_or(0)
    }
}

/// A station menu's slots: its own at `own`, then the player's inventory from `inventory`.
pub fn slots(own: &[(i32, i32)], inventory: (i32, i32)) -> Vec<Slot> {
    let mut slots: Vec<Slot> = own.iter().enumerate().map(|(i, &(x, y))| Slot::new(i, x, y)).collect();
    menus::standard_inventory(&mut slots, inventory.0, inventory.1);
    slots
}

/// `ItemCombinerScreen.extractBackground` up to the error icon: the darkening and `texture`.
pub fn background(gui: &mut Gui, b: &Base, texture: &'static str) {
    transparent_background(gui);
    blit(gui, texture, b.left, b.top, [0.0, 0.0], b.image_width, b.image_height);
}

/// Slot `i` holds a stack.
pub fn has(b: &Base, i: usize) -> bool {
    b.slots[i].has_item()
}

/// The 28 × 21 error icon at `(x, y)` in the window when inputs are in but no result is.
pub fn error_icon(gui: &mut Gui, b: &Base, sprite: &'static str, x: i32, y: i32, result: usize) {
    if (has(b, 0) || has(b, 1)) && !has(b, result) {
        gui.sprite(sprite, b.left + x, b.top + y, 28, 21);
    }
}
