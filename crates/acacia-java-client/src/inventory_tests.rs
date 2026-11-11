use super::*;
use acacia_java_proto::VarInt;

fn body(packet: &impl Wire) -> Vec<u8> {
    let mut out = Vec::new();
    packet.write(&mut out);
    out
}

fn stack(item: i32) -> Option<ItemStack> {
    Some(ItemStack::new(item, 1))
}

#[test]
fn content_fills_window_zero_and_ignores_other_containers() {
    let mut inventory = Inventory::default();
    let mut items = vec![None; SLOTS];
    items[HOTBAR_START + 2] = stack(10);
    items[OFFHAND] = stack(11);
    let packet = ContainerSetContent { container: VarInt(0), state_id: VarInt(4), items: items.clone(), carried: stack(12) };
    inventory.apply(from::CONTAINER_SET_CONTENT, &body(&packet)).unwrap();
    assert_eq!(inventory.hotbar(2), stack(10).as_ref());
    assert_eq!(inventory.offhand(), stack(11).as_ref());
    assert_eq!(inventory.cursor, stack(12));
    assert_eq!(inventory.state_id(), 4);

    let other = ContainerSetContent { container: VarInt(3), state_id: VarInt(9), items: vec![None; 63], carried: None };
    inventory.apply(from::CONTAINER_SET_CONTENT, &body(&other)).unwrap();
    assert_eq!((inventory.hotbar(2), inventory.state_id()), (stack(10).as_ref(), 4));
}

#[test]
fn set_slot_and_held_slot_move_the_main_hand() {
    let mut inventory = Inventory::default();
    let set = ContainerSetSlot { container: VarInt(0), state_id: VarInt(7), slot: 40, item: stack(20) };
    inventory.apply(from::CONTAINER_SET_SLOT, &body(&set)).unwrap();
    assert_eq!(inventory.held(), None);
    inventory.apply(from::SET_HELD_SLOT, &body(&SetHeldSlot { slot: VarInt(4) })).unwrap();
    assert_eq!((inventory.held(), inventory.selected, inventory.state_id()), (stack(20).as_ref(), 4, 7));
    inventory.apply(from::SET_HELD_SLOT, &body(&SetHeldSlot { slot: VarInt(9) })).unwrap();
    assert_eq!(inventory.selected, 4);
    let out_of_range = ContainerSetSlot { container: VarInt(0), state_id: VarInt(8), slot: 99, item: stack(21) };
    inventory.apply(from::CONTAINER_SET_SLOT, &body(&out_of_range)).unwrap();
    assert_eq!(inventory.state_id(), 8);
}

#[test]
fn player_inventory_indices_map_to_window_slots() {
    assert_eq!([0, 8, 9, 35, 36, 39, 40, 41, -1].map(window_slot), [Some(36), Some(44), Some(9), Some(35), Some(8), Some(5), Some(45), None, None]);
    let mut inventory = Inventory::default();
    inventory.apply(from::SET_PLAYER_INVENTORY, &body(&SetPlayerInventory { slot: VarInt(39), item: stack(30) })).unwrap();
    assert_eq!(inventory.slot(ARMOUR_HEAD), stack(30).as_ref());
    inventory.apply(from::SET_CURSOR_ITEM, &body(&SetCursorItem { item: stack(31) })).unwrap();
    assert_eq!(inventory.cursor, stack(31));
    assert_eq!(inventory.slot(CRAFT_RESULT), None);
}
