use super::*;
use crate::inventory::{HOTBAR_START, MAIN_START};

fn item(name: &str, count: i32) -> Option<ItemStack> {
    let id = Item::from_name(name).unwrap_or_else(|| panic!("{name}")).java_id();
    Some(ItemStack::new(i32::from(id), count))
}

fn with(slots: &[(usize, Option<ItemStack>)]) -> Inventory {
    let mut inventory = Inventory::default();
    for (i, s) in slots {
        inventory.slots[*i] = s.clone();
    }
    inventory
}

const HOT0: usize = HOTBAR_START;
const MAIN0: usize = MAIN_START;

#[test]
fn left_picks_up_and_places_reporting_changed_slots() {
    let mut inv = with(&[(MAIN0, item("dirt", 10))]);
    let packet = inv.click(Click::left(MAIN0 as i16));
    assert_eq!(packet.changed, vec![(MAIN0 as i16, None)]);
    assert_eq!(packet.carried.as_ref().map(|c| c.count), Some(10));
    let packet = inv.click(Click::left(HOT0 as i16));
    assert_eq!(packet.changed, vec![(HOT0 as i16, item("dirt", 10).as_ref().map(|s| HashedStack::of(s, &[])))]);
    assert_eq!((packet.carried, &inv.cursor), (None, &None));
}

#[test]
fn right_takes_half_rounded_up_and_places_one() {
    let mut inv = with(&[(MAIN0, item("dirt", 5))]);
    inv.click(Click::right(MAIN0 as i16));
    assert_eq!((inv.slot(MAIN0).map(|s| s.count), inv.cursor.as_ref().map(|s| s.count)), (Some(2), Some(3)));
    inv.click(Click::right(HOT0 as i16));
    assert_eq!((inv.slot(HOT0).map(|s| s.count), inv.cursor.as_ref().map(|s| s.count)), (Some(1), Some(2)));
}

#[test]
fn merging_stops_at_the_item_max_and_different_items_swap() {
    let mut inv = with(&[(MAIN0, item("ender_pearl", 10)), (HOT0, item("ender_pearl", 10)), (HOT0 + 1, item("dirt", 1))]);
    inv.click(Click::left(MAIN0 as i16));
    inv.click(Click::left(HOT0 as i16));
    assert_eq!((inv.slot(HOT0).map(|s| s.count), inv.cursor.as_ref().map(|s| s.count)), (Some(16), Some(4)));
    inv.click(Click::left(HOT0 as i16 + 1));
    assert_eq!((inv.slot(HOT0 + 1), inv.cursor.as_ref()), (item("ender_pearl", 4).as_ref(), item("dirt", 1).as_ref()));
}

#[test]
fn number_key_swaps_with_the_hotbar_and_q_throws() {
    let mut inv = with(&[(MAIN0 + 3, item("stone_pickaxe", 1)), (HOT0 + 2, item("dirt", 7))]);
    let packet = inv.click(Click::swap(MAIN0 as i16 + 3, 2));
    assert_eq!(packet.changed.len(), 2);
    assert_eq!((inv.slot(HOT0 + 2), inv.slot(MAIN0 + 3)), (item("stone_pickaxe", 1).as_ref(), item("dirt", 7).as_ref()));
    inv.click(Click::throw(MAIN0 as i16 + 3, false));
    assert_eq!(inv.slot(MAIN0 + 3).map(|s| s.count), Some(6));
    inv.click(Click::throw(MAIN0 as i16 + 3, true));
    assert_eq!(inv.slot(MAIN0 + 3), None);
}

#[test]
fn clicking_outside_drops_the_cursor() {
    let mut inv = with(&[]);
    inv.cursor = item("dirt", 3);
    inv.click(Click { slot: OUTSIDE, button: 1, input: ContainerInput::Pickup });
    assert_eq!(inv.cursor.as_ref().map(|s| s.count), Some(2));
    let packet = inv.click(Click { slot: OUTSIDE, button: 0, input: ContainerInput::Pickup });
    assert_eq!((packet.carried, packet.changed.len()), (None, 0));
}

#[test]
fn container_clicks_reach_the_player_inventory() {
    use acacia_java_proto::VarInt;
    use acacia_java_proto::container::OpenScreen;
    use acacia_java_proto::inventory::ContainerSetContent;

    let mut inv = with(&[]);
    inv.open(&OpenScreen { container: 3, menu: 2, title: Vec::new() });
    let mut items = vec![None; 27 + 36];
    items[0] = item("dirt", 5);
    items[27 + 27 + 1] = item("stone_shovel", 1);
    inv.fill_window(ContainerSetContent { container: VarInt(3), state_id: VarInt(8), items, carried: None });
    assert_eq!(inv.hotbar(1), item("stone_shovel", 1).as_ref());
    inv.click(Click::left(0));
    let packet = inv.click(Click::left(27 + 27 + 4));
    assert_eq!((packet.container, packet.state_id, packet.changed.len()), (3, 8, 1));
    assert_eq!((inv.hotbar(4), &inv.window.as_ref().unwrap().slots[0]), (item("dirt", 5).as_ref(), &None));
    inv.click(Click::swap(0, 1));
    assert_eq!((inv.hotbar(1), inv.menu_slot(0)), (None, item("stone_shovel", 1).as_ref()));
}
