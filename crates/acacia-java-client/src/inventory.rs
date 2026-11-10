//! The player's inventory as the server last described it, in `InventoryMenu` slot order.

use acacia_java_proto::ids::play::clientbound as from;
use acacia_java_proto::inventory::{ContainerSetContent, ContainerSetSlot, PLAYER_CONTAINER, SetCursorItem, SetHeldSlot, SetPlayerInventory};
use acacia_java_proto::container::{ContainerClose, ContainerSetData, OpenScreen};
use acacia_java_proto::item::ItemStack;
use acacia_java_proto::{Error as WireError, Wire};

use crate::window::Window;

pub const SLOTS: usize = 46;
pub const CRAFT_RESULT: usize = 0;
pub const ARMOUR_HEAD: usize = 5;
pub const MAIN_START: usize = 9;
pub const HOTBAR_START: usize = 36;
pub const OFFHAND: usize = 45;
const HOTBAR_SIZE: usize = 9;

/// Window 0: 0 craft result, 1-4 craft grid, 5-8 head to feet, 9-35 main, 36-44 hotbar, 45 offhand.
#[derive(Debug, Clone, PartialEq)]
pub struct Inventory {
    pub(crate) slots: [Option<ItemStack>; SLOTS],
    /// The hotbar slot in hand, 0-8.
    pub selected: u8,
    pub cursor: Option<ItemStack>,
    state_id: i32,
    /// The container screen the server opened, if any.
    pub window: Option<Window>,
    /// The enchantment registry's names by network id, from configuration.
    pub enchantments: std::sync::Arc<[String]>,
    /// Slots the last click predicted, by window-0 index.
    pub(crate) predicted: Vec<usize>,
    /// Predicted own-inventory slots the server re-sent unchanged: each is a click hash that missed.
    pub resent: u32,
}

impl Default for Inventory {
    fn default() -> Self {
        Self { slots: std::array::from_fn(|_| None), selected: 0, cursor: None, state_id: 0, window: None, enchantments: std::sync::Arc::from([]), predicted: Vec::new(), resent: 0 }
    }
}

/// `Inventory` index (0-8 hotbar, 9-35 main, 36-39 feet to head, 40 offhand) to window slot; body and saddle have none.
pub fn window_slot(inventory_index: i32) -> Option<usize> {
    match usize::try_from(inventory_index).ok()? {
        i @ 0..HOTBAR_SIZE => Some(HOTBAR_START + i),
        i @ MAIN_START..HOTBAR_START => Some(i),
        i @ 36..=39 => Some(ARMOUR_HEAD + 39 - i),
        40 => Some(OFFHAND),
        _ => None,
    }
}

impl Inventory {
    pub fn slot(&self, window_slot: usize) -> Option<&ItemStack> {
        self.slots.get(window_slot)?.as_ref()
    }

    /// Hotbar slot `i`, 0-8.
    pub fn hotbar(&self, i: usize) -> Option<&ItemStack> {
        if i < HOTBAR_SIZE { self.slot(HOTBAR_START + i) } else { None }
    }

    /// The main-hand stack.
    pub fn held(&self) -> Option<&ItemStack> {
        self.hotbar(usize::from(self.selected))
    }

    pub fn offhand(&self) -> Option<&ItemStack> {
        self.slot(OFFHAND)
    }

    /// The state id to echo in container clicks.
    pub fn state_id(&self) -> i32 {
        self.state_id
    }

    /// Overwrites a slot, as the server's slot packets do.
    pub fn set_slot(&mut self, window_slot: usize, item: Option<ItemStack>) {
        if let Some(slot) = self.slots.get_mut(window_slot) {
            *slot = item;
        }
    }

    /// Applies the inventory and container packets of `ClientPacketListener`.
    pub(crate) fn apply(&mut self, id: i32, mut body: &[u8]) -> Result<(), WireError> {
        match id {
            from::CONTAINER_SET_CONTENT => {
                let packet = ContainerSetContent::read(&mut body)?;
                if packet.container.0 != PLAYER_CONTAINER {
                    self.fill_window(packet);
                } else {
                    let mut items = packet.items.into_iter();
                    self.slots = std::array::from_fn(|_| items.next().flatten());
                    self.cursor = packet.carried;
                    self.state_id = packet.state_id.0;
                }
            }
            from::CONTAINER_SET_SLOT => {
                let packet = ContainerSetSlot::read(&mut body)?;
                if packet.container.0 != PLAYER_CONTAINER {
                    self.set_window_slot(packet);
                } else {
                    if let Ok(slot) = usize::try_from(packet.slot) {
                        self.resent += u32::from(self.predicted.contains(&slot) && self.slots.get(slot) == Some(&packet.item));
                        self.set_slot(slot, packet.item);
                    }
                    self.state_id = packet.state_id.0;
                }
            }
            from::SET_PLAYER_INVENTORY => {
                let packet = SetPlayerInventory::read(&mut body)?;
                if let Some(slot) = window_slot(packet.slot.0) {
                    self.set_slot(slot, packet.item);
                }
            }
            from::OPEN_SCREEN => self.open(&OpenScreen::read(&mut body)?),
            from::CONTAINER_SET_DATA => self.set_window_data(ContainerSetData::read(&mut body)?),
            from::CONTAINER_CLOSE => self.close(ContainerClose::read(&mut body)?.container.0),
            from::SET_CURSOR_ITEM => self.cursor = SetCursorItem::read(&mut body)?.item,
            from::SET_HELD_SLOT => {
                let slot = SetHeldSlot::read(&mut body)?.slot.0;
                if (0..HOTBAR_SIZE as i32).contains(&slot) {
                    self.selected = slot as u8;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;
