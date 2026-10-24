use acacia_client::proto::nbt::{Nbt, Value};
use acacia_client::proto::types::{WindowID, WindowIDVarint};

use super::*;
use crate::state::queries::test_support::{content, empty_item, fixtures, item, named_item, raw, response, slot_packet, update_trade};

const ME: Me = Me { runtime_entity_id: 7, unique_entity_id: -7 };
const CHEST: WindowIDVarint = WindowIDVarint::First;
const AT: BlockCoordinates = BlockCoordinates { x: 10, y: 64, z: -3 };

fn open_at(containers: &mut Containers, window_id: WindowID, coordinates: BlockCoordinates) {
    let open = ContainerOpen { window_id, window_type: WindowType::Container, coordinates, runtime_entity_id: -1 };
    containers.apply(&raw(&open), &ME).unwrap();
}

fn close(containers: &mut Containers, window_id: WindowID, server: bool) {
    containers.apply(&raw(&ContainerClose { window_id, window_type: WindowType::Container, server }), &ME).unwrap();
}

fn named_block(containers: &mut Containers, position: BlockCoordinates, name: Option<&str>) {
    let mut entries = vec![("id".into(), Value::String("Chest".into()))];
    entries.extend(name.map(|n| ("CustomName".into(), Value::String(n.into()))));
    let nbt = Nbt { name: Default::default(), value: Value::Compound(entries) };
    containers.apply(&raw(&BlockEntityData { position, nbt: nbt.into() }), &ME).unwrap();
}

#[test]
fn open_fill_update_close() {
    let mut containers = Containers::default();
    open_at(&mut containers, WindowID::First, AT);
    let open = containers.open.as_ref().unwrap();
    assert_eq!((open.window_id, open.window_type), (1, WindowType::Container));
    assert_eq!((open.position.as_ref(), open.entity, open.title.as_deref()), (Some(&AT), None, None));

    let mut items = vec![empty_item(); 27];
    items[13] = named_item(304, 1, "§aBuy Diamond", &["$100"]);
    containers.apply(&raw(&content(CHEST, items)), &ME).unwrap();
    containers.apply(&raw(&content(WindowIDVarint::Inventory, vec![item(1, 1, 1)])), &ME).unwrap();
    containers.apply(&raw(&slot_packet(CHEST, 2, item(5, 8, 50))), &ME).unwrap();
    containers.apply(&raw(&response(true, ContainerSlotType::Container, 2, 3, 51, "")), &ME).unwrap();
    containers.apply(&raw(&response(true, ContainerSlotType::Hotbar, 13, 0, 0, "")), &ME).unwrap();

    let open = containers.open.as_ref().unwrap();
    assert_eq!(open.slots.len(), 27);
    assert_eq!(open.slot(13).unwrap().custom_name.as_deref(), Some("§aBuy Diamond"));
    assert_eq!((open.slot(2).unwrap().count, open.slot(2).unwrap().stack_network_id), (3, Some(51)));
    assert_eq!(open.iter().count(), 2);
    assert_eq!(containers.contents, 1, "only the open window's contents count");

    close(&mut containers, WindowID::First, true);
    assert!(containers.open.is_none());
    assert_eq!(containers.last_window_id, Some(1));
}

#[test]
fn late_close_for_another_window_keeps_the_open_one() {
    let mut containers = Containers::default();
    open_at(&mut containers, WindowID::Inventory, AT);
    open_at(&mut containers, WindowID::First, AT);
    close(&mut containers, WindowID::Inventory, false);
    close(&mut containers, WindowID::from_raw(2), true);
    assert_eq!(containers.open.as_ref().map(|c| c.window_id), Some(1));
    close(&mut containers, WindowID::First, false);
    assert!(containers.open.is_none());

    open_at(&mut containers, WindowID::Inventory, AT);
    close(&mut containers, WindowID::Inventory, false);
    assert!(containers.open.is_none(), "own inventory (window 0) closes by its id");
    open_at(&mut containers, WindowID::First, AT);
    close(&mut containers, WindowID::None, true);
    assert!(containers.open.is_none(), "WindowID::None closes whatever is open");
}

#[test]
fn title_comes_from_the_block_entity_name_before_or_after_the_open() {
    let mut containers = Containers::default();
    named_block(&mut containers, AT, Some("§8Auction House (Page 1)"));
    named_block(&mut containers, BlockCoordinates { x: 0, y: 0, z: 0 }, None);
    open_at(&mut containers, WindowID::First, AT);
    assert_eq!(containers.open.as_ref().unwrap().title.as_deref(), Some("§8Auction House (Page 1)"));

    named_block(&mut containers, AT, Some("§8Auction House (Page 2)"));
    assert_eq!(containers.open.as_ref().unwrap().title.as_deref(), Some("§8Auction House (Page 2)"));

    let elsewhere = BlockCoordinates { x: 1, y: 2, z: 3 };
    open_at(&mut containers, WindowID::from_raw(2), elsewhere.clone());
    assert_eq!(containers.open.as_ref().unwrap().title, None);
    named_block(&mut containers, elsewhere, Some("late"));
    assert_eq!(containers.open.as_ref().unwrap().title.as_deref(), Some("late"));

    for x in 0..TITLES as i32 {
        named_block(&mut containers, BlockCoordinates { x: 100 + x, y: 0, z: 0 }, Some("filler"));
    }
    assert_eq!(containers.titles.len(), TITLES);
    open_at(&mut containers, WindowID::from_raw(3), AT);
    assert_eq!(containers.open.as_ref().unwrap().title, None, "old names are dropped");
}

#[test]
fn entity_container_and_slot_growth() {
    let mut containers = Containers::default();
    let open = ContainerOpen {
        window_id: WindowID::First,
        window_type: WindowType::Horse,
        coordinates: BlockCoordinates { x: 0, y: 0, z: 0 },
        runtime_entity_id: 42,
    };
    containers.apply(&raw(&open), &ME).unwrap();
    containers.apply(&raw(&slot_packet(CHEST, 4, item(9, 1, 2))), &ME).unwrap();
    let open = containers.open.as_ref().unwrap();
    assert_eq!((open.entity, open.position.clone(), open.title.clone()), (Some(42), None, None));
    assert_eq!(open.slots.len(), 5);
    assert_eq!(open.slot(4).unwrap().network_id, 9);

    close(&mut containers, WindowID::First, true);
    containers.apply(&raw(&update_trade(Vec::new())), &ME).unwrap();
    assert_eq!(containers.open.as_ref().unwrap().title.as_deref(), Some("entity.villager.librarian"));
}

#[test]
fn decodes_fixtures() {
    let mut containers = Containers::default();
    for packet in fixtures::<ContainerOpen>().into_iter().chain(fixtures::<InventoryContent>()) {
        containers.apply(&packet, &ME).unwrap();
    }
    for packet in fixtures::<ContainerClose>() {
        containers.apply(&packet, &ME).unwrap();
    }
}
