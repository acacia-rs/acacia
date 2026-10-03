use acacia_client::proto::types::{ContainerSlotType as T, WindowType};

use super::super::plan::Plan;
use super::super::quick::quick_move_ops;
use super::super::request::{check_status, response_for};
use super::*;
use crate::items::{Op, SlotRef};
use crate::ActionError;

const ID: i32 = -11;

/// Runs one request the way `Bot::item_stack_request` does: plan, then the server's response
/// goes through the trackers, then the plan commits if it succeeded.
fn run(state: &mut GameState, ops: &[Op], ok: bool, slots: &[(T, u8, u8, i32)]) -> Result<(), ActionError> {
    let plan = Plan::build(state, screen(state), ops)?;
    let packet = response_packet(vec![response(ID, ok, slots)]);
    state.apply(&packet).unwrap();
    let response = response_for(&packet, ID).unwrap();
    check_status(&response)?;
    plan.commit(state, &response);
    Ok(())
}

fn main(state: &GameState, slot: usize) -> &ItemStack {
    &state.inventory.main[slot]
}

#[test]
fn split_into_empty_slot() {
    let mut state = state_with(&[(0, stack(5, 10, 7))]);
    let op = Op::Transfer { from: SlotRef::Main(0), to: SlotRef::Main(12), count: 4 };
    run(&mut state, &[op], true, &[(T::Hotbar, 0, 6, 7), (T::Inventory, 12, 4, 20)]).unwrap();
    assert_eq!(*main(&state, 0), stack(5, 6, 7));
    assert_eq!(*main(&state, 12), stack(5, 4, 20));
}

#[test]
fn move_whole_stack_keeps_item_data() {
    let mut named = stack(5, 3, 7);
    named.custom_name = Some("§aSword".into());
    named.lore = vec!["sharp".into()];
    let mut state = state_with(&[(4, named.clone())]);
    run(&mut state, &[Op::Transfer { from: SlotRef::Main(4), to: SlotRef::Main(30), count: 3 }], true, &[
        (T::Hotbar, 4, 0, 0),
        (T::Inventory, 30, 3, 7),
    ])
    .unwrap();
    assert!(main(&state, 4).is_empty());
    assert_eq!(*main(&state, 30), named);
}

#[test]
fn merge_onto_matching_stack() {
    let mut state = state_with(&[(0, stack(5, 10, 7)), (1, stack(5, 30, 8))]);
    run(&mut state, &[Op::Transfer { from: SlotRef::Main(0), to: SlotRef::Main(1), count: 10 }], true, &[
        (T::Hotbar, 0, 0, 0),
        (T::Hotbar, 1, 40, 8),
    ])
    .unwrap();
    assert!(main(&state, 0).is_empty());
    assert_eq!(*main(&state, 1), stack(5, 40, 8));
}

#[test]
fn swap_exchanges_items_not_just_counts() {
    let mut state = state_with(&[(0, stack(5, 10, 7)), (9, stack(6, 2, 8))]);
    run(&mut state, &[Op::Swap { a: SlotRef::Main(0), b: SlotRef::Main(9) }], true, &[
        (T::Hotbar, 0, 2, 8),
        (T::Inventory, 9, 10, 7),
    ])
    .unwrap();
    assert_eq!(*main(&state, 0), stack(6, 2, 8));
    assert_eq!(*main(&state, 9), stack(5, 10, 7));
}

#[test]
fn drop_part_and_all() {
    let mut state = state_with(&[(0, stack(5, 10, 7))]);
    run(&mut state, &[Op::Drop { from: SlotRef::Main(0), count: 3 }], true, &[(T::Hotbar, 0, 7, 7)]).unwrap();
    assert_eq!(*main(&state, 0), stack(5, 7, 7));
    run(&mut state, &[Op::Drop { from: SlotRef::Main(0), count: 7 }], true, &[(T::Hotbar, 0, 0, 0)]).unwrap();
    assert!(main(&state, 0).is_empty());
}

#[test]
fn container_to_inventory_and_back() {
    let mut state = GameState::default();
    open_container(&mut state, WindowType::Container, vec![ItemStack::default(), stack(5, 16, 30)]);
    let op = Op::Transfer { from: SlotRef::Container(1), to: SlotRef::Main(9), count: 16 };
    run(&mut state, &[op], true, &[(T::Container, 1, 0, 0), (T::HotbarAndInventory, 9, 16, 31)]).unwrap();
    assert!(SlotRef::Container(1).stack(&state).unwrap().is_empty());
    assert_eq!(*main(&state, 9), stack(5, 16, 31));

    let op = Op::Transfer { from: SlotRef::Main(9), to: SlotRef::Container(0), count: 6 };
    run(&mut state, &[op], true, &[(T::HotbarAndInventory, 9, 10, 31), (T::Container, 0, 6, 32)]).unwrap();
    assert_eq!(*SlotRef::Container(0).stack(&state).unwrap(), stack(5, 6, 32));
    assert_eq!(main(&state, 9).count, 10);
}

#[test]
fn pick_up_into_cursor() {
    let mut state = GameState::default();
    open_container(&mut state, WindowType::Container, vec![stack(304, 1, 40)]);
    let op = Op::Transfer { from: SlotRef::Container(0), to: SlotRef::Cursor, count: 1 };
    run(&mut state, &[op], true, &[(T::Container, 0, 0, 0), (T::Cursor, 0, 1, 40)]).unwrap();
    assert_eq!(*state.inventory.cursor(), stack(304, 1, 40));
}

#[test]
fn rejected_request_leaves_state_untouched() {
    let mut state = state_with(&[(0, stack(5, 10, 7)), (1, stack(6, 2, 8))]);
    let before = state.inventory.main.clone();
    let ops = [Op::Transfer { from: SlotRef::Main(0), to: SlotRef::Main(12), count: 4 }, Op::Swap { a: SlotRef::Main(1), b: SlotRef::Main(2) }];
    let err = run(&mut state, &ops, false, &[]).unwrap_err();
    assert!(matches!(err, ActionError::Rejected(_)), "{err:?}");
    assert_eq!(state.inventory.main, before);
}

#[test]
fn quick_move_merges_then_fills_an_empty_slot() {
    let mut state = state_with(&[(0, stack(5, 60, 7))]);
    open_container(&mut state, WindowType::Container, vec![stack(5, 20, 30)]);
    let ops = quick_move_ops(&state, SlotRef::Container(0)).unwrap();
    assert_eq!(ops, [
        Op::Transfer { from: SlotRef::Container(0), to: SlotRef::Main(0), count: 4 },
        Op::Transfer { from: SlotRef::Container(0), to: SlotRef::Main(1), count: 16 },
    ]);

    for (slot, item) in state.inventory.main.iter_mut().enumerate().skip(1) {
        *item = stack(6, 1, 100 + slot as i32);
    }
    state.inventory.main[20] = stack(5, 50, 8);
    let ops = quick_move_ops(&state, SlotRef::Container(0)).unwrap();
    assert_eq!(ops, [
        Op::Transfer { from: SlotRef::Container(0), to: SlotRef::Main(0), count: 4 },
        Op::Transfer { from: SlotRef::Container(0), to: SlotRef::Main(20), count: 14 },
    ]);
    // Both actions start from the same source stack.
    Plan::build(&state, screen(&state), &ops).unwrap();

    state.inventory.main[0] = stack(6, 1, 99);
    state.inventory.main[20] = stack(6, 1, 98);
    assert!(matches!(quick_move_ops(&state, SlotRef::Container(0)), Err(ActionError::NotPossible(_))));
}

#[test]
fn single_items_only_merge_when_proven_stackable() {
    let mut state = state_with(&[(0, stack(9, 1, 7)), (1, stack(5, 1, 8))]);
    open_container(&mut state, WindowType::Furnace, vec![stack(9, 1, 30), ItemStack::default(), stack(5, 2, 31)]);
    let to = |ops: Vec<Op>| if let [Op::Transfer { to, .. }] = ops[..] { to } else { panic!("{ops:?}") };
    assert_eq!(to(quick_move_ops(&state, SlotRef::Container(0)).unwrap()), SlotRef::Main(2), "two swords never stack");
    assert_eq!(to(quick_move_ops(&state, SlotRef::Container(2)).unwrap()), SlotRef::Main(1), "a stack of two does");
}

#[test]
fn quick_move_between_hotbar_and_inventory() {
    let state = state_with(&[(2, stack(5, 1, 7)), (9, stack(6, 1, 8)), (0, stack(7, 1, 9))]);
    assert_eq!(quick_move_ops(&state, SlotRef::Main(2)).unwrap(), [Op::Transfer { from: SlotRef::Main(2), to: SlotRef::Main(10), count: 1 }]);
    assert_eq!(quick_move_ops(&state, SlotRef::Main(9)).unwrap(), [Op::Transfer { from: SlotRef::Main(9), to: SlotRef::Main(1), count: 1 }]);
}
