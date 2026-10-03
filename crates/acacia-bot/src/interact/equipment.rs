//! The held item as the server sees it (`MobEquipment`). Vanilla sends it whenever the selected
//! slot or its content changes, preceded by `Interact` mouse-over-nothing when the slot or the item
//! type changed (capture 2026-10-02, ~40 cases). Server-side changes are resent from
//! `crate::reflex`.

use super::wire;
use crate::state::{Inventory, ItemStack};
use crate::{ActionError, Bot};

impl Bot {
    /// Selects hotbar slot 0..=8. Servers do not echo the selection back, so it is also applied to
    /// the inventory tracker as if they had.
    pub fn select_hotbar(&mut self, slot: u8) -> Result<(), ActionError> {
        if slot >= Inventory::HOTBAR_SLOTS {
            return Err(ActionError::NotPossible(format!("hotbar slot {slot} out of range")));
        }
        self.state.inventory.select(slot);
        self.equip_now();
        Ok(())
    }

    /// Sends the held stack now and records it as what the server was told.
    pub(crate) fn equip_now(&mut self) {
        let inv = &self.state.inventory;
        let (slot, item) = (inv.selected_hotbar_slot, inv.held().clone());
        if needs_mouse_over(self.reflexes.equipped.as_ref(), slot, &item) {
            self.client.send(&wire::mouse_over_nothing());
            self.reflexes.hovered = None;
        }
        self.client.send(&wire::mob_equipment(self.runtime_id(), slot, &item));
        self.reflexes.equipped = Some((slot, item));
    }
}

/// Whether vanilla would send mouse-over-nothing before this `MobEquipment`: a new slot or item type,
/// not a count, damage or NBT change.
pub(crate) fn needs_mouse_over(last: Option<&(u8, ItemStack)>, slot: u8, item: &ItemStack) -> bool {
    last.is_none_or(|(s, i)| *s != slot || i.network_id != item.network_id)
}

/// Same item as far as `MobEquipment` shows it (stack ids are not sent).
pub(crate) fn same_content(a: &ItemStack, b: &ItemStack) -> bool {
    (a.network_id, a.count, a.metadata, a.block_runtime_id) == (b.network_id, b.count, b.metadata, b.block_runtime_id) && a.nbt == b.nbt
}
