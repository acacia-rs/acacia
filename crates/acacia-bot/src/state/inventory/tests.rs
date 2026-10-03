use acacia_client::proto::packets::{InventoryContent, InventorySlot, ItemStackResponse, MobEquipment, PlayerHotbar};
use acacia_client::proto::types::{ContainerSlotType, ItemV4, WindowID, WindowIDVarint};

use super::*;
use crate::state::queries::test_support::{content, empty_item, fixtures, item, named_item, raw, response, slot_packet};

const ME: Me = Me { runtime_entity_id: 7, unique_entity_id: -7 };

fn inventory_with(items: Vec<ItemV4>) -> Inventory {
    let mut inv = Inventory::default();
    inv.apply(&raw(&content(WindowIDVarint::Inventory, items)), &ME).unwrap();
    inv
}

#[test]
fn content_fills_main_and_decodes_names() {
    let mut items = vec![empty_item(); 36];
    items[0] = item(5, 64, 100);
    items[10] = named_item(304, 3, "§6Shop Diamond", &["§7Buy: $10", "line 2"]);
    items[20] = item(5, 1, 101);
    let inv = inventory_with(items);

    assert_eq!(inv.slot(0, 0).unwrap().count, 64);
    assert_eq!(inv.slot(0, 0).unwrap().stack_network_id, Some(100));
    let named = inv.slot(Inventory::WINDOW_INVENTORY, 10).unwrap();
    assert_eq!(named.custom_name.as_deref(), Some("§6Shop Diamond"));
    assert_eq!(named.lore, ["§7Buy: $10", "line 2"]);
    assert!(named.nbt.is_some());
    assert!(inv.slot(0, 1).unwrap().is_empty());
    assert_eq!(inv.count_of(5), 65);
    assert_eq!(inv.find(|s| s.network_id == 304).map(|(w, i, _)| (w, i)), Some((0, 10)));
    assert!(inv.find(|s| s.network_id == 999).is_none());
}

#[test]
fn slot_updates_armor_offhand_and_cursor() {
    let mut inv = Inventory::default();
    inv.apply(&raw(&slot_packet(WindowIDVarint::Armor, 1, item(10, 1, 1))), &ME).unwrap();
    inv.apply(&raw(&slot_packet(WindowIDVarint::Offhand, 0, item(11, 1, 2))), &ME).unwrap();
    inv.apply(&raw(&slot_packet(WindowIDVarint::Ui, 0, item(12, 5, 3))), &ME).unwrap();
    inv.apply(&raw(&slot_packet(WindowIDVarint::Ui, 30, item(13, 5, 4))), &ME).unwrap();
    inv.apply(&raw(&slot_packet(WindowIDVarint::Inventory, 99, item(14, 1, 5))), &ME).unwrap();

    assert_eq!(inv.armor[1].network_id, 10);
    assert_eq!(inv.offhand.network_id, 11);
    assert_eq!(inv.cursor().network_id, 12);
    assert_eq!(inv.slot(Inventory::WINDOW_UI, 0).unwrap().count, 5);
    assert_eq!(inv.ui[30].network_id, 13);
    assert_eq!(inv.iter().count(), 4);
}

#[test]
fn server_transactions_rewrite_container_slots() {
    use crate::interact::legacy::{rewrite_slot, SlotChange};
    let mut inv = inventory_with(vec![item(521, 1, 91)]);
    let (old, new) = (inv.main[0].clone(), ItemStack { network_id: 522, ..inv.main[0].clone() });
    inv.apply(&raw(&rewrite_slot(None, &SlotChange { slot: 0, old: &old, new: &new })), &ME).unwrap();
    assert_eq!(inv.main[0].network_id, 522, "creative actions are not slots");
    assert_eq!(inv.iter().count(), 1);
}

#[test]
fn hotbar_selection_for_own_entity_only() {
    let mut inv = inventory_with(vec![item(1, 1, 1), item(2, 1, 2), item(3, 1, 3)]);
    let equip = |id, slot| MobEquipment { runtime_entity_id: id, item: empty_item(), slot, selected_slot: slot, window_id: WindowID::Inventory };
    inv.apply(&raw(&equip(8, 2)), &ME).unwrap();
    assert_eq!(inv.selected_hotbar_slot, 0);
    inv.apply(&raw(&equip(7, 2)), &ME).unwrap();
    assert_eq!(inv.held().network_id, 3);
    inv.apply(&raw(&equip(7, 12)), &ME).unwrap();
    assert_eq!(inv.selected_hotbar_slot, 2);

    let hotbar = |slot, select| PlayerHotbar { selected_slot: slot, window_id: WindowID::Inventory, select_slot: select };
    inv.apply(&raw(&hotbar(1, false)), &ME).unwrap();
    assert_eq!(inv.selected_hotbar_slot, 2);
    inv.apply(&raw(&hotbar(1, true)), &ME).unwrap();
    assert_eq!(inv.held().network_id, 2);
}

#[test]
fn stack_response_updates_ok_slots_and_ignores_errors() {
    let mut inv = inventory_with(vec![item(5, 64, 100), item(6, 10, 101)]);
    inv.apply(&raw(&response(true, ContainerSlotType::Hotbar, 0, 32, 200, "Renamed")), &ME).unwrap();
    inv.apply(&raw(&response(true, ContainerSlotType::HotbarAndInventory, 1, 0, 0, "")), &ME).unwrap();
    inv.apply(&raw(&response(false, ContainerSlotType::Hotbar, 0, 1, 300, "")), &ME).unwrap();
    inv.apply(&raw(&response(true, ContainerSlotType::Cursor, 0, 3, 400, "")), &ME).unwrap();

    let first = &inv.main[0];
    assert_eq!((first.network_id, first.count, first.stack_network_id), (5, 32, Some(200)));
    assert_eq!(first.custom_name.as_deref(), Some("Renamed"));
    assert!(inv.main[1].is_empty());
    assert!(inv.cursor().is_empty(), "an empty slot gets no item type from a response");
}

#[test]
fn decodes_fixtures() {
    let mut inv = Inventory::default();
    for packet in fixtures::<InventoryContent>().into_iter().chain(fixtures::<InventorySlot>()).chain(fixtures::<ItemStackResponse>()) {
        inv.apply(&packet, &ME).unwrap();
    }
    assert_eq!(inv.main.len(), Inventory::MAIN_SLOTS);
}
