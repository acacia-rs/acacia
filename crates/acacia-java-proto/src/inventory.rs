//! Clientbound container and inventory contents.

use crate::item::{Components, ItemStack};
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

/// `ServerboundSetCreativeModeSlotPacket`: a window-0 slot set outright, its stack in
/// `OPTIONAL_UNTRUSTED_STREAM_CODEC` (length-delimited components).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SetCreativeModeSlot {
    pub slot: i16,
    pub item: Option<ItemStack>,
}

impl Wire for SetCreativeModeSlot {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let slot = i16::read(input)?;
        let count = VarInt::read(input)?.0;
        if count <= 0 {
            return Ok(Self { slot, item: None });
        }
        let item = VarInt::read(input)?.0;
        Ok(Self { slot, item: Some(ItemStack { item, count, components: Components::read_delimited(input)? }) })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.slot.write(out);
        match &self.item {
            Some(stack) if stack.count > 0 => {
                VarInt(stack.count).write(out);
                VarInt(stack.item).write(out);
                stack.components.write_delimited(out);
            }
            _ => VarInt(0).write(out),
        }
    }
}

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

    #[test]
    fn creative_slots_delimit_each_component() {
        let mut stack = ItemStack::new(900, 3);
        stack.components.damage = Some(5);
        let packet = SetCreativeModeSlot { slot: 36, item: Some(stack) };
        let mut out = Vec::new();
        packet.write(&mut out);
        // slot, count, item (2 bytes), 1 added 0 removed, damage (3) 1 byte long: 5.
        assert_eq!(out, [0, 36, 3, 0x84, 7, 1, 0, 3, 1, 5]);
        assert_eq!(SetCreativeModeSlot::read(&mut &out[..]), Ok(packet));
        let mut out = Vec::new();
        SetCreativeModeSlot { slot: 9, item: None }.write(&mut out);
        assert_eq!(out, [0, 9, 0]);
    }
}
