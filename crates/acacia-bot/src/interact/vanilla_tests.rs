//! Click and hotbar packet shapes against the vanilla capture (docs/research/vanilla-actions-2026-10-02.md).

use acacia_client::proto::nbt::{List, Nbt, Value};
use acacia_client::proto::packets::{Interact, InteractActionId, PlayerAction};
use acacia_client::proto::types::{Action, TransactionActionsItemSourceType as Source, TransactionTransactionData as Data, TransactionTransactionType};

use super::equipment::{needs_mouse_over, same_content};
use super::item_use::{one_used, use_on_result};
use super::legacy::{rewrite_slot, with_slot_change, SlotChange};
use super::wire::{self, Hand};
use super::{to_wire, Face};
use crate::state::queries::test_support::raw;
use crate::state::ItemStack;

fn stack(network_id: i32, count: u16) -> ItemStack {
    ItemStack { network_id, count, stack_network_id: Some(90), ..ItemStack::default() }
}

#[test]
fn result_position_is_the_neighbour_only_for_empty_hands_blocks_and_signs() {
    let (pos, face) = ([1, 70, 4], Face::Up);
    assert_eq!(use_on_result(&ItemStack::default(), false, pos, face), [1, 71, 4], "empty hand");
    assert_eq!(use_on_result(&ItemStack { block_runtime_id: 425, ..stack(85, 6) }, false, pos, face), [1, 71, 4], "fence");
    assert_eq!(use_on_result(&stack(361, 2), true, pos, face), [1, 71, 4], "sign");
    assert_eq!(use_on_result(&stack(421, 1), false, pos, face), pos, "bed");
    assert_eq!(use_on_result(&stack(619, 1), false, pos, face), pos, "pickaxe");
}

#[test]
fn item_use_on_actions() {
    let p: PlayerAction = raw(&wire::item_use_on(1, Action::StartItemUseOn, [1, 70, 4], [1, 71, 4], 1)).decode().unwrap();
    assert_eq!((p.action, p.position.y, p.result_position.y, p.face), (Action::StartItemUseOn, 70, 71, 1));
    let p: PlayerAction = raw(&wire::item_use_on(1, Action::StopItemUseOn, [1, 71, 4], [0, 0, 0], 0)).decode().unwrap();
    assert_eq!((p.position.y, p.result_position.x, p.result_position.y, p.face), (71, 0, 0, 0));
}

#[test]
fn placing_lists_the_held_slot_change() {
    let sign = stack(361, 2);
    let left = one_used(&sign);
    assert_eq!((left.count, one_used(&stack(421, 1)).is_empty()), (1, true));
    let hand = Hand { slot: 3, item: to_wire(&sign), eye: [1.06, 72.62, 3.17] };
    let click = wire::click_block(hand, [1, 70, 4], Face::Up, Face::Up.click_offset(), 7);
    let packet = with_slot_change(click, Some(-70), &SlotChange { slot: 3, old: &sign, new: &left });
    let t = raw(&packet).decode::<acacia_client::proto::packets::InventoryTransaction>().unwrap().transaction;
    assert_eq!(t.legacy.legacy_request_id, -70);
    let listed = t.legacy.legacy_transactions.unwrap();
    assert_eq!((listed[0].container_id, listed[0].changed_slots[0].slot_id), (29, 3));
    let a = &t.actions[0];
    assert_eq!((a.source_type, a.window_id, a.slot, a.old_item.count, a.new_item.count), (Source::Container, Some(0), 3, 2, 1));
    assert!(!a.old_item.has_stack_id && !a.new_item.has_stack_id);
    assert!(matches!(t.transaction_data, Data::ItemUse(_)));

    let bed = stack(421, 1);
    let t = with_slot_change(wire::click_air(Hand { slot: 0, item: to_wire(&bed), eye: [0.0; 3] }), None, &SlotChange { slot: 0, old: &bed, new: &one_used(&bed) })
        .transaction;
    assert_eq!((t.legacy.legacy_request_id, t.legacy.legacy_transactions, t.actions[0].new_item.network_id), (0, None, 0));
}

#[test]
fn book_rewrites_carry_the_creative_pair() {
    let old = stack(521, 1);
    let pages = List { tag: 0, items: Vec::new() };
    let new = ItemStack { nbt: Some(Nbt { name: String::new(), value: Value::Compound(vec![("pages".into(), Value::List(pages))]) }), ..old.clone() };
    let packet = rewrite_slot(Some(-78), &SlotChange { slot: 0, old: &old, new: &new });
    let t = raw(&packet).decode::<acacia_client::proto::packets::InventoryTransaction>().unwrap().transaction;
    assert_eq!((t.transaction_type, t.transaction_data), (TransactionTransactionType::Normal, Data::Normal));
    let shape: Vec<_> = t.actions.iter().map(|a| (a.source_type, a.window_id, a.slot, a.old_item.network_id, a.new_item.network_id)).collect();
    assert_eq!(shape, [(Source::Container, Some(0), 0, 521, 521), (Source::Creative, None, 0, 0, 521), (Source::Creative, None, 1, 521, 0)]);
    assert_eq!(t.actions[1].new_item, t.actions[0].old_item, "creative slot 0 takes the old book");
    assert_eq!(t.actions[2].old_item, t.actions[0].new_item, "creative slot 1 hands out the new one");
}

#[test]
fn mouse_over_goes_before_equipment_only_for_a_new_slot_or_item_type() {
    let sword = stack(318, 1);
    assert!(needs_mouse_over(None, 0, &sword));
    assert!(needs_mouse_over(Some(&(1, sword.clone())), 0, &sword), "slot switch");
    assert!(needs_mouse_over(Some(&(0, stack(421, 1))), 0, &ItemStack::default()), "bed placed: hand empty");
    assert!(!needs_mouse_over(Some(&(3, stack(361, 2))), 3, &stack(361, 1)), "sign placed: one fewer");
    assert!(same_content(&sword, &ItemStack { stack_network_id: Some(7), ..sword.clone() }), "stack ids are not sent");
    assert!(!same_content(&sword, &ItemStack { count: 2, ..sword.clone() }));

    let nothing: Interact = raw(&wire::mouse_over_nothing()).decode().unwrap();
    assert_eq!((nothing.action_id, nothing.target_entity_id, nothing.has_position), (InteractActionId::MouseOverEntity, 0, false));
    let over: Interact = raw(&wire::mouse_over(23, [1.0, 71.5, 2.0])).decode().unwrap();
    assert_eq!((over.target_entity_id, over.has_position, over.position.map(|p| p.y)), (23, true, Some(71.5)));
}
