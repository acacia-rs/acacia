//! Opt-in answers to the client's inventory traffic, enough for crafting and container verbs: every item stack
//! request is accepted (no slot changes listed, so the client's prediction stands), the own inventory screen opens
//! on request, blocks the test registered open as containers, and client closes are echoed as vanilla does.
//! Container contents stay what the test set; requests do not change them.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use acacia_proto::packets::{
    ContainerClose, ContainerOpen, Interact, InteractActionId, InventoryContent, InventoryTransaction, ItemStackRequest,
    ItemStackResponse,
};
use acacia_proto::types::{
    BlockCoordinates, ContainerSlotType, FullContainerName, ItemStackResponsesItem, ItemStackResponsesItemStatus, ItemV4,
    ItemV4Extra, TransactionTransactionData, TransactionUseItemActionType, WindowID, WindowIDVarint, WindowType,
};
use acacia_proto::{Packet, RawPacket};
use acacia_world::BlockRegistry;

use crate::peer::encode;
use crate::server::Command;
use crate::FakeServer;

/// Window ids BDS hands out for opened blocks cycle below this.
const MAX_WINDOW_ID: i64 = 100;

#[derive(Clone)]
struct ContainerBlock {
    kind: WindowType,
    slots: Vec<ItemV4>,
}

/// The containers [`FakeServer::serve_inventory`] opens; cheap to clone.
#[derive(Clone, Default)]
pub struct InventoryServer {
    containers: Arc<Mutex<HashMap<[i32; 3], ContainerBlock>>>,
}

impl InventoryServer {
    /// Makes using the block at `pos` open a `kind` window holding `items` (`(slot, item name, count)`); a chest
    /// (`Container`) has 27 slots, a furnace, blast furnace or smoker 3, other kinds none.
    pub fn add_container(&self, server: &FakeServer, pos: [i32; 3], kind: WindowType, items: &[(usize, &str, u16)]) {
        let size = match kind {
            WindowType::Container => 27,
            WindowType::Furnace | WindowType::BlastFurnace | WindowType::Smoker => 3,
            _ => 0,
        };
        let mut slots = vec![empty(); size];
        for &(slot, name, count) in items {
            let id = server.item_id(name).unwrap_or_else(|| panic!("{name} is not in the script's item registry"));
            let block = BlockRegistry::vanilla().find(name, "").map_or(0, |b| BlockRegistry::vanilla().network_hash(b));
            slots[slot] = ItemV4 { network_id: id, count, has_stack_id: true, stack_id: Some(2000 + slot as i32), block_runtime_id: block, ..empty() };
        }
        self.containers.lock().expect("containers lock").insert(pos, ContainerBlock { kind, slots });
    }

    fn answer(&self, packet: &RawPacket, next_window: &mut i64) -> Vec<bytes::Bytes> {
        match packet.id {
            ItemStackRequest::ID => {
                let Ok(p) = packet.decode::<ItemStackRequest>() else { return Vec::new() };
                let accept = |request_id| ItemStackResponsesItem { status: ItemStackResponsesItemStatus::Ok, request_id, containers: Some(Vec::new()) };
                vec![encode(&ItemStackResponse { responses: p.requests.iter().map(|r| accept(r.request_id)).collect() })]
            }
            Interact::ID => match packet.decode::<Interact>() {
                Ok(p) if p.action_id == InteractActionId::OpenInventory => {
                    let (window_id, window_type, coordinates) = (WindowID::Inventory, WindowType::Inventory, BlockCoordinates { x: 0, y: 0, z: 0 });
                    vec![encode(&ContainerOpen { window_id, window_type, coordinates, runtime_entity_id: p.target_entity_id as i64 })]
                }
                _ => Vec::new(),
            },
            InventoryTransaction::ID => {
                let Ok(p) = packet.decode::<InventoryTransaction>() else { return Vec::new() };
                let TransactionTransactionData::ItemUse(u) = p.transaction.transaction_data else { return Vec::new() };
                let b = u.block_position;
                let block = self.containers.lock().expect("containers lock").get(&[b.x, b.y, b.z]).cloned();
                match block {
                    Some(c) if u.action_type == TransactionUseItemActionType::ClickBlock => {
                        *next_window = *next_window % (MAX_WINDOW_ID - 1) + 1;
                        open_packets(*next_window, b, c)
                    }
                    _ => Vec::new(),
                }
            }
            ContainerClose::ID => match packet.decode::<ContainerClose>() {
                Ok(p) if !p.server => vec![encode(&ContainerClose { window_id: p.window_id, window_type: WindowType::None, server: false })],
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }
}

impl FakeServer {
    /// Starts answering the client's inventory traffic (see the module docs) until the server stops.
    pub fn serve_inventory(&self) -> InventoryServer {
        let inventory = InventoryServer::default();
        let (shared, mut changes, commands) = (self.shared.clone(), self.changes.clone(), self.commands.clone());
        let mut seen = shared.lock().expect("fake server lock").received.len();
        let answers = inventory.clone();
        tokio::spawn(async move {
            let mut window = 0;
            while changes.changed().await.is_ok() {
                let fresh: Vec<RawPacket> = {
                    let s = shared.lock().expect("fake server lock");
                    let fresh = s.received[seen..].iter().map(|r| r.packet.clone()).collect();
                    seen = s.received.len();
                    fresh
                };
                for packet in fresh.iter().flat_map(|p| answers.answer(p, &mut window)) {
                    let _ = commands.send(Command::Send(packet));
                }
            }
        });
        inventory
    }
}

fn open_packets(window: i64, pos: BlockCoordinates, c: ContainerBlock) -> Vec<bytes::Bytes> {
    let open = ContainerOpen { window_id: WindowID::from_raw(window), window_type: c.kind, coordinates: pos, runtime_entity_id: -1 };
    if c.slots.is_empty() {
        return vec![encode(&open)];
    }
    let container = FullContainerName { container_id: ContainerSlotType::Container, dynamic_container_id: None };
    let content = InventoryContent { window_id: WindowIDVarint::from_raw(window), input: c.slots, container, storage_item: empty() };
    vec![encode(&open), encode(&content)]
}

fn empty() -> ItemV4 {
    ItemV4 { network_id: 0, count: 0, metadata: 0, has_stack_id: false, stack_id: None, block_runtime_id: 0, extra: ItemV4Extra::Default(None) }
}
