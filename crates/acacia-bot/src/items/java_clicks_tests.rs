use acacia_java_proto::container::ContainerInput;
use acacia_java_proto::item::ItemStack;
use acacia_world::neutral::Item;

use super::*;

fn stack(name: &str, count: i32) -> Option<ItemStack> {
    Some(ItemStack::new(i32::from(Item::from_name(name).unwrap().java_id()), count))
}

fn inventory(slots: &[(SlotRef, Option<ItemStack>)]) -> Inventory {
    let mut inv = Inventory::default();
    for (slot, s) in slots {
        inv.set_slot(window(&Inventory::default(), *slot).unwrap() as usize, s.clone());
    }
    inv
}

/// The plan's clicks and the inventory they leave.
fn run(inv: &Inventory, ops: &[Op]) -> (Vec<Click>, Inventory) {
    let clicks = plan(inv, ops).unwrap();
    let mut after = inv.clone();
    clicks.iter().for_each(|&c| drop(after.click(c)));
    (clicks, after)
}

fn count(inv: &Inventory, slot: SlotRef) -> i32 {
    inv.menu_slot(window(inv, slot).unwrap() as usize).map_or(0, |s| s.count)
}

#[test]
fn whole_stack_moves_with_two_left_clicks() {
    let inv = inventory(&[(SlotRef::Main(20), stack("dirt", 30))]);
    let (clicks, after) = run(&inv, &[Op::Transfer { from: SlotRef::Main(20), to: SlotRef::Main(2), count: 30 }]);
    assert_eq!(clicks, vec![Click::left(20), Click::left(38)]);
    assert_eq!((count(&after, SlotRef::Main(20)), count(&after, SlotRef::Main(2))), (0, 30));
}

#[test]
fn partial_moves_right_click_and_put_the_rest_back() {
    let inv = inventory(&[(SlotRef::Main(20), stack("dirt", 30)), (SlotRef::Main(2), stack("dirt", 5))]);
    let (clicks, after) = run(&inv, &[Op::Transfer { from: SlotRef::Main(20), to: SlotRef::Main(2), count: 3 }]);
    assert_eq!(clicks.len(), 5);
    assert_eq!((count(&after, SlotRef::Main(20)), count(&after, SlotRef::Main(2)), after.cursor.is_none()), (27, 8, true));
}

#[test]
fn overflowing_merges_are_refused() {
    let inv = inventory(&[(SlotRef::Main(20), stack("ender_pearl", 10)), (SlotRef::Main(2), stack("ender_pearl", 10))]);
    assert!(plan(&inv, &[Op::Transfer { from: SlotRef::Main(20), to: SlotRef::Main(2), count: 10 }]).is_err());
    assert!(plan(&inv, &[Op::Transfer { from: SlotRef::Main(20), to: SlotRef::Main(3), count: 10 }]).is_ok());
}

#[test]
fn hotbar_swaps_use_the_number_key() {
    let inv = inventory(&[(SlotRef::Main(20), stack("stone_pickaxe", 1)), (SlotRef::Main(4), stack("dirt", 9))]);
    let (clicks, after) = run(&inv, &[Op::Swap { a: SlotRef::Main(4), b: SlotRef::Main(20) }]);
    assert_eq!(clicks, vec![Click { slot: 20, button: 4, input: ContainerInput::Swap }]);
    assert_eq!(after.hotbar(4), stack("stone_pickaxe", 1).as_ref());
}

#[test]
fn drops_throw_one_at_a_time_or_the_whole_stack() {
    let inv = inventory(&[(SlotRef::Main(0), stack("dirt", 4))]);
    let (clicks, _) = run(&inv, &[Op::Drop { from: SlotRef::Main(0), count: 2 }]);
    assert_eq!(clicks, vec![Click::throw(36, false); 2]);
    let (clicks, after) = run(&inv, &[Op::Drop { from: SlotRef::Main(0), count: 4 }]);
    assert_eq!((clicks, count(&after, SlotRef::Main(0))), (vec![Click::throw(36, true)], 0));
}

#[test]
fn container_slots_need_an_open_container_and_deletes_are_creative_only() {
    let inv = inventory(&[(SlotRef::Main(0), stack("dirt", 4))]);
    assert!(plan(&inv, &[Op::Transfer { from: SlotRef::Main(0), to: SlotRef::Container(0), count: 4 }]).is_err());
    assert!(plan(&inv, &[Op::Destroy { from: SlotRef::Main(0), count: 4 }]).is_err());
}

#[test]
fn crafting_grids_are_menu_slots_one_on() {
    let mut inv = inventory(&[(SlotRef::Main(0), stack("oak_planks", 2))]);
    let to_grid = |cell| [Op::Transfer { from: SlotRef::Main(0), to: SlotRef::Ui(cell), count: 1 }];
    assert_eq!(run(&inv, &to_grid(ui::CRAFTING_2X2 + 2)).0, vec![Click::left(36), Click::right(3), Click::left(36)]);
    assert!(plan(&inv, &to_grid(ui::CRAFTING_3X3)).is_err());
    inv.window = Some(acacia_java_client::window::Window { id: 4, menu: CRAFTING_MENU, title: None, slots: vec![None; 10], state_id: 1, data: Vec::new(), merchant: None });
    assert_eq!(run(&inv, &to_grid(ui::CRAFTING_3X3 + 4)).0, vec![Click::left(10 + 27), Click::right(5), Click::left(10 + 27)]);
}

#[test]
fn chest_moves_number_the_player_slots_after_the_chest() {
    let mut inv = inventory(&[(SlotRef::Main(0), stack("dirt", 4)), (SlotRef::Main(10), stack("cobblestone", 8))]);
    let mut chest = vec![None; 27];
    chest[5] = stack("stone_shovel", 1);
    inv.window = Some(acacia_java_client::window::Window { id: 2, menu: 2, title: None, slots: chest, state_id: 1, data: Vec::new(), merchant: None });
    let ops = [
        Op::Transfer { from: SlotRef::Main(0), to: SlotRef::Container(0), count: 4 },
        Op::Transfer { from: SlotRef::Main(10), to: SlotRef::Container(1), count: 8 },
        Op::Swap { a: SlotRef::Container(5), b: SlotRef::Main(3) },
    ];
    let (clicks, after) = run(&inv, &ops);
    assert_eq!(clicks[..4], [Click::left(27 + 27), Click::left(0), Click::left(27 + 1), Click::left(1)]);
    assert_eq!(clicks[4], Click { slot: 5, button: 3, input: ContainerInput::Swap });
    assert_eq!((count(&after, SlotRef::Container(1)), after.hotbar(3)), (8, stack("stone_shovel", 1).as_ref()));
    assert!(plan(&inv, &[Op::Swap { a: SlotRef::Offhand, b: SlotRef::Main(3) }]).is_err());
}
