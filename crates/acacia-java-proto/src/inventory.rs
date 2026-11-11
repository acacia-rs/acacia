//! Clientbound container and inventory contents.

use crate::item::ItemStack;
use crate::packets::packet;
use crate::{Error, VarInt, Wire};

/// The player's own menu, `InventoryMenu.CONTAINER_ID`.
pub const PLAYER_CONTAINER: i32 = 0;

packet!(ContainerSetContent { container: VarInt, state_id: VarInt, items: Vec<Option<ItemStack>>, carried: Option<ItemStack> });
packet!(ContainerSetSlot { container: VarInt, state_id: VarInt, slot: i16, item: Option<ItemStack> });
// `slot` is an `Inventory` index: 0-8 hotbar, 9-35 main, 36-39 feet to head, 40 offhand.
packet!(SetPlayerInventory { slot: VarInt, item: Option<ItemStack> });
packet!(SetCursorItem { item: Option<ItemStack> });
packet!(SetHeldSlot { slot: VarInt });

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_slot_reads_vanillas_layout() {
        // Container 0, state 5, slot 36, one item 1 with an empty patch.
        let body = [0, 5, 0, 36, 1, 1, 0, 0];
        let packet = ContainerSetSlot::read(&mut &body[..]).unwrap();
        assert_eq!(packet, ContainerSetSlot { container: VarInt(0), state_id: VarInt(5), slot: 36, item: Some(ItemStack::new(1, 1)) });
        let mut out = Vec::new();
        packet.write(&mut out);
        assert_eq!(out, body);
    }

    #[test]
    fn set_content_round_trips_with_empty_slots() {
        let packet = ContainerSetContent {
            container: VarInt(0),
            state_id: VarInt(1),
            items: vec![None, Some(ItemStack::new(800, 64)), None],
            carried: Some(ItemStack::new(2, 1)),
        };
        let mut out = Vec::new();
        packet.write(&mut out);
        assert_eq!(ContainerSetContent::read(&mut &out[..]), Ok(packet));
    }
}
