use acacia_client::proto::packets::ItemStackRequest as ItemStackRequestPacket;
use acacia_client::proto::types::{
    ContainerSlotType as T, ItemStackRequestActionsItemContent as Content, ItemStackRequestActionsItemTypeId as TypeId,
    ItemStackRequestCause, StackRequestSlotInfo, WindowType,
};

use super::super::plan::Plan;
use super::super::request::{check_status, request_packet, response_for, RequestIds, Texts};
use super::super::slot::{BlockKind, Screen};
use super::*;
use crate::items::{Op, SlotRef};
use crate::ActionError;

fn round_trip(state: &GameState, screen: Screen, ops: &[Op]) -> ItemStackRequestPacket {
    let packet = Plan::build(state, screen, ops).unwrap().request(-5);
    let decoded: ItemStackRequestPacket = raw(&packet).decode().unwrap();
    assert_eq!(decoded, packet);
    decoded
}

fn info(kind: T, slot: u8, stack_id: i32) -> StackRequestSlotInfo {
    StackRequestSlotInfo { slot_type: FullContainerName { container_id: kind, dynamic_container_id: None }, slot, stack_id }
}

#[test]
fn place_between_player_slots_round_trips() {
    let state = state_with(&[(0, stack(5, 10, 7)), (20, stack(9, 1, 8))]);
    let packet = round_trip(&state, screen(&state), &[Op::Transfer { from: SlotRef::Main(0), to: SlotRef::Main(12), count: 4 }]);
    let request = &packet.requests[0];
    assert_eq!(request.request_id, -5);
    let action = &request.actions[0];
    assert_eq!((action.type_id, action.legacy_type_id), (TypeId::Place, 1));
    let Content::Place(place) = &action.content else { panic!("expected place: {action:?}") };
    assert_eq!(place.count, 4);
    assert_eq!(place.source, info(T::Hotbar, 0, 7));
    assert_eq!(place.destination, info(T::Inventory, 12, 0));
}

#[test]
fn slot_names_per_screen() {
    let mut state = state_with(&[(3, stack(5, 2, 7)), (30, stack(6, 1, 9))]);
    state.inventory.offhand = stack(8, 1, 11);
    state.inventory.armor[1] = stack(12, 1, 12);
    let own = screen(&state);
    assert_eq!(SlotRef::Main(3).slot_info(&own, false, SlotRef::Main(3).stack(&state).unwrap()), info(T::Hotbar, 3, 7));
    assert_eq!(SlotRef::Main(3).slot_info(&own, true, SlotRef::Main(3).stack(&state).unwrap()), info(T::HotbarAndInventory, 3, 7));
    assert_eq!(SlotRef::Main(30).slot_info(&own, false, SlotRef::Main(30).stack(&state).unwrap()), info(T::Inventory, 30, 9));
    assert_eq!(SlotRef::Offhand.slot_info(&own, false, &state.inventory.offhand), info(T::Offhand, 1, 11));
    assert_eq!(SlotRef::Armor(1).slot_info(&own, false, &state.inventory.armor[1]), info(T::Armor, 1, 12));
    assert_eq!(SlotRef::Cursor.wire(&own, false), (T::Cursor, 0));

    // BDS opens the own inventory as the next window id; it is still the own screen.
    open_container(&mut state, WindowType::Inventory, Vec::new());
    state.containers.open.as_mut().unwrap().window_id = 24;
    assert_eq!(screen(&state).window_type, WindowType::Inventory);

    open_container(&mut state, WindowType::Container, vec![ItemStack::default(); 27]);
    let chest = screen(&state);
    assert_eq!(SlotRef::Main(3).wire(&chest, false), (T::Hotbar, 3));
    assert_eq!(SlotRef::Main(3).wire(&chest, true), (T::HotbarAndInventory, 3));
    assert_eq!(SlotRef::Main(30).wire(&chest, true), (T::HotbarAndInventory, 30));
    assert_eq!(SlotRef::Container(26).wire(&chest, false), (T::Container, 26));
    let barrel = Screen::of(&state, BlockKind::from_name("minecraft:barrel"));
    assert_eq!(SlotRef::Container(4).wire(&barrel, false), (T::Barrel, 4));
    let shulker = Screen::of(&state, BlockKind::from_name("minecraft:red_shulker_box"));
    assert_eq!(SlotRef::Container(4).wire(&shulker, false), (T::Shulker, 4));

    open_container(&mut state, WindowType::BlastFurnace, vec![ItemStack::default(); 3]);
    let furnace = screen(&state);
    let kinds: Vec<_> = (0..3).map(|i| SlotRef::Container(i).wire(&furnace, false).0).collect();
    assert_eq!(kinds, [T::BlastFurnaceIngredient, T::FurnaceFuel, T::FurnaceOutput]);

    open_container(&mut state, WindowType::BrewingStand, vec![ItemStack::default(); 5]);
    let brewing = screen(&state);
    let kinds: Vec<_> = (0..5).map(|i| SlotRef::Container(i).wire(&brewing, false).0).collect();
    assert_eq!(kinds, [T::BrewingInput, T::BrewingResult, T::BrewingResult, T::BrewingResult, T::BrewingFuel]);
}

/// The furnace clicks of the 2026-10-02 capture: in from the hotbar, out by shift-click onto the
/// ingots in slot 11, and a cursor drop into the inventory.
#[test]
fn player_slots_are_named_like_vanilla_in_a_container() {
    let mut state = state_with(&[(2, stack(516, 2, 6)), (11, stack(307, 8, 24))]);
    state.inventory.ui[0] = stack(307, 1, 50);
    open_container(&mut state, WindowType::Furnace, vec![ItemStack::default(), ItemStack::default(), stack(307, 1, 73)]);
    let sources = |ops: &[Op]| -> Vec<StackRequestSlotInfo> {
        round_trip(&state, screen(&state), ops).requests[0]
            .actions
            .iter()
            .flat_map(|a| match &a.content {
                Content::Place(p) => [p.source.clone(), p.destination.clone()],
                other => panic!("{other:?}"),
            })
            .collect()
    };
    assert_eq!(sources(&[Op::Transfer { from: SlotRef::Main(2), to: SlotRef::Container(0), count: 2 }]), [
        info(T::Hotbar, 2, 6),
        info(T::FurnaceIngredient, 0, 0)
    ]);
    assert_eq!(sources(&[Op::Transfer { from: SlotRef::Container(2), to: SlotRef::Main(11), count: 1 }]), [
        info(T::FurnaceOutput, 2, 73),
        info(T::HotbarAndInventory, 11, 24)
    ]);
    assert_eq!(sources(&[Op::Transfer { from: SlotRef::Cursor, to: SlotRef::Main(34), count: 1 }]), [
        info(T::Cursor, 0, 50),
        info(T::Inventory, 34, 0)
    ]);
}

#[test]
fn every_action_kind_round_trips() {
    let mut state = state_with(&[(0, stack(5, 10, 7)), (1, stack(6, 3, 8)), (2, stack(9, 5, 10))]);
    open_container(&mut state, WindowType::Container, vec![stack(5, 2, 30), ItemStack::default()]);
    let ops = [
        Op::Transfer { from: SlotRef::Main(0), to: SlotRef::Cursor, count: 10 },
        Op::Transfer { from: SlotRef::Cursor, to: SlotRef::Container(0), count: 10 },
        Op::Swap { a: SlotRef::Main(1), b: SlotRef::Container(1) },
        Op::Drop { from: SlotRef::Main(2), count: 2 },
        Op::Destroy { from: SlotRef::Main(2), count: 3 },
    ];
    let packet = round_trip(&state, screen(&state), &ops);
    let types: Vec<_> = packet.requests[0].actions.iter().map(|a| (a.type_id, a.legacy_type_id)).collect();
    assert_eq!(types, [(TypeId::Take, 0), (TypeId::Place, 1), (TypeId::Swap, 2), (TypeId::Drop, 3), (TypeId::Destroy, 4)]);
    let actions = &packet.requests[0].actions;
    let Content::Take(take) = &actions[0].content else { panic!() };
    assert_eq!((take.source.clone(), take.destination.clone()), (info(T::Hotbar, 0, 7), info(T::Cursor, 0, 0)));
    // The second action sees the cursor as the first left it, named by the request id like vanilla.
    let Content::Place(place) = &actions[1].content else { panic!() };
    assert_eq!((place.source.clone(), place.destination.clone()), (info(T::Cursor, 0, -5), info(T::Container, 0, 30)));
    let Content::Drop(drop) = &actions[3].content else { panic!() };
    assert!(!drop.randomly);
}

#[test]
fn invalid_ops_are_refused_before_sending() {
    let state = state_with(&[(0, stack(5, 10, 7)), (1, stack(6, 3, 8))]);
    let screen = screen(&state);
    let refused = |ops: &[Op]| matches!(Plan::build(&state, screen, ops), Err(ActionError::NotPossible(_)));
    assert!(refused(&[Op::Transfer { from: SlotRef::Main(0), to: SlotRef::Main(1), count: 1 }]), "different item");
    assert!(refused(&[Op::Transfer { from: SlotRef::Main(0), to: SlotRef::Main(2), count: 11 }]), "too many");
    assert!(refused(&[Op::Transfer { from: SlotRef::Main(0), to: SlotRef::Main(0), count: 1 }]), "same slot");
    assert!(refused(&[Op::Drop { from: SlotRef::Main(5), count: 1 }]), "empty source");
    assert!(refused(&[Op::Transfer { from: SlotRef::Container(0), to: SlotRef::Main(5), count: 1 }]), "no container");
    assert!(refused(&[Op::Swap { a: SlotRef::Main(4), b: SlotRef::Main(5) }]), "both empty");
    assert!(refused(&[]));
}

#[test]
fn matches_response_by_request_id() {
    let packet = response_packet(vec![response(-5, true, &[]), response(-7, false, &[])]);
    let found = response_for(&packet, -7).unwrap();
    assert_eq!(found.request_id, -7);
    assert!(matches!(check_status(&found), Err(ActionError::Rejected(_))));
    assert!(check_status(&response_for(&packet, -5).unwrap()).is_ok());
    assert!(response_for(&packet, -9).is_none());
    let not_a_response = raw(&request_packet(-7, Vec::new(), Texts::default()));
    assert!(response_for(&not_a_response, -7).is_none());
}

#[test]
fn request_ids_count_down_from_minus_three_per_session() {
    let mut ids = RequestIds::default();
    assert_eq!([ids.next(), ids.next(), ids.next()], [-3, -5, -7]);
    assert_eq!(RequestIds::default().next(), -3, "every session starts afresh");
    let mut ids = RequestIds { next: i32::MIN + 1 };
    assert_eq!([ids.next(), ids.next()], [i32::MIN + 1, -3]);
}

#[test]
fn texts_default_to_no_cause() {
    assert_eq!(Texts::default().cause, ItemStackRequestCause::Unknown(-1));
}
