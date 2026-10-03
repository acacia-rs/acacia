mod encoding;
mod prediction;

use acacia_client::proto::packets::ItemStackResponse;
use acacia_client::proto::types::{
    BlockCoordinates, ContainerSlotType, FullContainerName, ItemStackResponsesItem, ItemStackResponsesItemContainersItem,
    ItemStackResponsesItemContainersItemSlotsItem, ItemStackResponsesItemStatus, WindowType,
};
use acacia_client::proto::{encode_packet, Packet, RawPacket};
use bytes::BytesMut;

use super::slot::{BlockKind, Screen};
use crate::state::{Container, GameState, ItemStack};

pub(super) fn raw<T: Packet>(packet: &T) -> RawPacket {
    let mut buf = BytesMut::new();
    encode_packet(packet, &mut buf);
    RawPacket::parse(buf.freeze()).unwrap()
}

pub(super) fn stack(network_id: i32, count: u16, stack_id: i32) -> ItemStack {
    ItemStack { network_id, count, stack_network_id: Some(stack_id), ..ItemStack::default() }
}

/// A state whose main inventory holds `items` as `(slot, stack)`.
pub(super) fn state_with(items: &[(usize, ItemStack)]) -> GameState {
    let mut state = GameState::default();
    for (slot, item) in items {
        state.inventory.main[*slot] = item.clone();
    }
    state
}

pub(super) fn open_container(state: &mut GameState, window_type: WindowType, slots: Vec<ItemStack>) {
    state.containers.open = Some(Container {
        window_id: 1,
        window_type,
        position: Some(BlockCoordinates { x: 0, y: 64, z: 0 }),
        entity: None,
        slots,
    });
}

pub(super) fn screen(state: &GameState) -> Screen {
    Screen::of(state, BlockKind::Other)
}

/// One `ItemStackResponse` entry; `slots` are `(container, slot, count, stack id)`.
pub(super) fn response(request_id: i32, ok: bool, slots: &[(ContainerSlotType, u8, u8, i32)]) -> ItemStackResponsesItem {
    let containers = slots
        .iter()
        .map(|&(kind, slot, count, stack_id)| ItemStackResponsesItemContainersItem {
            slot_type: FullContainerName { container_id: kind, dynamic_container_id: None },
            slots: vec![ItemStackResponsesItemContainersItemSlotsItem {
                slot,
                hotbar_slot: slot,
                count,
                item_stack_id: (count > 0).then_some(stack_id),
                custom_name: String::new(),
                filtered_custom_name: String::new(),
                durability_correction: 0,
            }],
        })
        .collect();
    let status = if ok { ItemStackResponsesItemStatus::Ok } else { ItemStackResponsesItemStatus::Error };
    ItemStackResponsesItem { status, request_id, containers: ok.then_some(containers) }
}

pub(super) fn response_packet(entries: Vec<ItemStackResponsesItem>) -> RawPacket {
    raw(&ItemStackResponse { responses: entries })
}
