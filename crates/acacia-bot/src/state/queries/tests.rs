use acacia_client::proto::nbt::Nbt;
use acacia_client::proto::packets::{ContainerOpen, ItemRegistry as ItemRegistryPacket, PlayerHotbar};
use acacia_client::proto::types::{BlockCoordinates, ItemstatesItem, ItemstatesItemVersion, WindowID, WindowIDVarint, WindowType};

use super::test_support::{content, empty_item, item, raw};
use crate::state::{GameState, Trackers};

fn registry() -> ItemRegistryPacket {
    let entry = |name: &str, runtime_id| ItemstatesItem {
        name: name.into(),
        runtime_id,
        component_based: false,
        version: ItemstatesItemVersion::Legacy,
        nbt: Nbt::default(),
    };
    ItemRegistryPacket {
        itemstates: vec![entry("minecraft:stone", 1), entry("minecraft:diamond", 304), entry("minecraft:emerald", 500)],
    }
}

fn state() -> GameState {
    let mut state = GameState::new(Trackers::default(), 7);
    state.apply(&raw(&registry())).unwrap();
    let mut items = vec![empty_item(); 36];
    items[0] = item(1, 64, 1);
    items[1] = item(304, 3, 2);
    items[9] = item(1, 10, 3);
    items[20] = item(77, 2, 4);
    state.apply(&raw(&content(WindowIDVarint::Inventory, items))).unwrap();
    state
}

#[test]
fn names_and_held_item() {
    let mut state = state();
    assert_eq!(state.item_name(state.held_item()), Some("minecraft:stone"));
    state.apply(&raw(&PlayerHotbar { selected_slot: 1, window_id: WindowID::Inventory, select_slot: true })).unwrap();
    assert_eq!(state.item_name(state.held_item()), Some("minecraft:diamond"));
    assert_eq!(state.item_name(&state.inventory.main[20]), None);
}

#[test]
fn find_item_searches_inventory_then_open_container() {
    let mut state = state();
    assert_eq!(state.find_item("minecraft:diamond").map(|(w, s, i)| (w, s, i.count)), Some((0, 1, 3)));
    assert!(state.find_item("minecraft:emerald").is_none());
    assert!(state.find_item("minecraft:unknown").is_none());

    let open = ContainerOpen {
        window_id: WindowID::First,
        window_type: WindowType::Container,
        coordinates: BlockCoordinates { x: 0, y: 0, z: 0 },
        runtime_entity_id: -1,
    };
    state.apply(&raw(&open)).unwrap();
    state.apply(&raw(&content(WindowIDVarint::First, vec![empty_item(), item(500, 7, 9)]))).unwrap();
    assert_eq!(state.find_item("minecraft:emerald").map(|(w, s, i)| (w, s, i.count)), Some((1, 1, 7)));
    assert_eq!(state.inventory.count_of(500), 0);
}

#[test]
fn summary_totals_by_identifier() {
    assert_eq!(
        state().inventory_summary(),
        [("#77".to_owned(), 2), ("minecraft:diamond".to_owned(), 3), ("minecraft:stone".to_owned(), 74)]
    );
}
