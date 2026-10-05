mod anvil;
mod beacon;
mod cartography;
mod crafting;
mod grid;
mod stations;
mod trade;

use acacia_client::proto::nbt::{Nbt, Raw};
use acacia_client::proto::packets::{ItemRegistry as ItemRegistryPacket, ItemStackRequest as ItemStackRequestPacket};
use acacia_client::proto::types::{
    ContainerSlotType, FullContainerName, ItemStackRequest, ItemStackRequestActionsItemTypeId as TypeId, ItemstatesItem,
    ItemstatesItemVersion, StackRequestSlotInfo, WindowType,
};

use crate::items::craft::Craft;
use crate::items::{BlockKind, Op, Plan, Screen};
use crate::state::queries::test_support::raw;
use crate::state::{Container, GameState, ItemStack};

pub(super) const REQUEST: i32 = -7;

/// Identifiers and network ids every test registry knows.
pub(super) const ITEMS: &[(&str, i16)] = &[
    ("minecraft:oak_planks", 5),
    ("minecraft:stick", 320),
    ("minecraft:coal", 302),
    ("minecraft:torch", 50),
    ("minecraft:crafting_table", 58),
    ("minecraft:diamond_sword", 310),
    ("minecraft:netherite_sword", 743),
    ("minecraft:netherite_ingot", 742),
    ("minecraft:netherite_upgrade_smithing_template", 790),
    ("minecraft:lapis_lazuli", 400),
    ("minecraft:stone", 1),
    ("minecraft:stone_slab", 44),
    ("minecraft:white_banner", 567),
    ("minecraft:red_dye", 351),
    ("minecraft:emerald", 388),
    ("minecraft:paper", 339),
    ("minecraft:iron_ingot", 265),
    ("minecraft:diamond", 264),
    ("minecraft:filled_map", 423),
    ("minecraft:empty_map", 526),
    ("minecraft:glass_pane", 102),
    ("minecraft:packed_ice", 174),
    ("minecraft:oak_fence", 85),
    ("minecraft:enchanted_book", 403),
    ("minecraft:book", 340),
];

pub(super) fn id(name: &str) -> i32 {
    i32::from(ITEMS.iter().find(|(n, _)| *n == name).unwrap().1)
}

pub(super) fn stack(name: &str, count: u16, stack_id: i32) -> ItemStack {
    ItemStack { network_id: id(name), count, stack_network_id: Some(stack_id), ..ItemStack::default() }
}

/// A state with the test registry and `main` as `(slot, stack)` in the main inventory.
pub(super) fn state(main: &[(usize, ItemStack)]) -> GameState {
    let mut state = GameState::default();
    let itemstates = ITEMS
        .iter()
        .map(|&(name, runtime_id)| ItemstatesItem {
            name: name.into(),
            runtime_id,
            component_based: false,
            version: ItemstatesItemVersion::Legacy,
            nbt: Raw::default(),
        })
        .collect();
    state.apply(&raw(&ItemRegistryPacket { itemstates })).unwrap();
    for (slot, item) in main {
        state.inventory.main[*slot] = item.clone();
    }
    state
}

pub(super) fn open(state: &mut GameState, window_type: WindowType) {
    state.containers.open = Some(Container { window_id: 3, window_type, position: None, entity: None, slots: Vec::new() });
}

/// The encoded request (decoded back from its bytes) for a craft plan.
pub(super) fn encode(state: &GameState, (craft, ops): &(Craft, Vec<Op>)) -> ItemStackRequest {
    let screen = Screen::of(state, BlockKind::Other);
    let packet = Plan::craft(state, screen, craft, ops).unwrap().request(REQUEST);
    let decoded: ItemStackRequestPacket = raw(&packet).decode().unwrap();
    assert_eq!(decoded, packet);
    decoded.requests.into_iter().next().unwrap()
}

/// `(variant, legacy id)` of every action.
pub(super) fn kinds(request: &ItemStackRequest) -> Vec<(TypeId, u8)> {
    request.actions.iter().map(|a| (a.type_id, a.legacy_type_id)).collect()
}

pub(super) fn info(kind: ContainerSlotType, slot: u8, stack_id: i32) -> StackRequestSlotInfo {
    StackRequestSlotInfo { slot_type: FullContainerName { container_id: kind, dynamic_container_id: None }, slot, stack_id }
}
