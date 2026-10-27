use std::collections::{HashMap, VecDeque};

use acacia_client::proto::packets::{
    BlockEntityData, ContainerClose, ContainerOpen, ContainerSetData, InventoryContent, InventorySlot, ItemStackResponse,
    UpdateTrade,
};
use acacia_client::proto::nbt::{root_string, Network};
use acacia_client::proto::types::{BlockCoordinates, ContainerSlotType, WindowType};
use acacia_client::proto::{DecodeError, Packet, RawPacket};

use super::inventory::successful_slots;
use super::{Inventory, ItemStack, Me};

/// Block-entity names remembered for titling windows opened later.
const TITLES: usize = 8;

/// One open container window (chest, shop GUI, furnace, ...).
#[derive(Debug, Clone, PartialEq)]
pub struct Container {
    pub window_id: i32,
    pub window_type: WindowType,
    /// The block that was opened, unless it is an entity container.
    pub position: Option<BlockCoordinates>,
    /// Unique id of the entity that was opened (horse, chest minecart, ...).
    pub entity: Option<i64>,
    /// The window's title with `§` codes: the opened block entity's `CustomName` (Geyser titles its
    /// fake GUI blocks this way) or the trade screen's display name. `None` for default names.
    pub title: Option<String>,
    /// Filled by the `InventoryContent` that follows the open; empty until then.
    pub slots: Vec<ItemStack>,
}

impl Container {
    pub fn slot(&self, slot: usize) -> Option<&ItemStack> {
        self.slots.get(slot)
    }

    /// Every non-empty stack as `(slot, stack)`.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &ItemStack)> {
        self.slots.iter().enumerate().filter(|(_, s)| !s.is_empty())
    }
}

/// The open container. Bedrock has at most one open besides the player's own inventory; opening
/// the own inventory (window 0, type `Inventory`) also shows up here, with no slots.
#[derive(Debug, Default)]
pub struct Containers {
    pub open: Option<Container>,
    /// The open window's `ContainerSetData` properties: a furnace's 0 cook ticks, 1 lit time,
    /// 2 lit duration; a brewing stand's 0 brew time, 1 fuel, 2 fuel total.
    pub data: HashMap<i32, i32>,
    /// The last window opened, whether or not it is still open.
    pub last_window_id: Option<i32>,
    /// Bumped by every `InventoryContent` for the open window.
    pub(crate) contents: u64,
    /// Recent block-entity `CustomName`s by position, newest last.
    titles: VecDeque<([i32; 3], String)>,
}

impl Containers {
    pub const PACKETS: &'static [u32] = &[
        ContainerOpen::ID,
        ContainerClose::ID,
        ContainerSetData::ID,
        InventoryContent::ID,
        InventorySlot::ID,
        ItemStackResponse::ID,
        UpdateTrade::ID,
        BlockEntityData::ID,
    ];

    pub fn apply(&mut self, packet: &RawPacket, _me: &Me) -> Result<(), DecodeError> {
        match packet.id {
            ContainerOpen::ID => {
                let p: ContainerOpen = packet.decode()?;
                // Named runtime_entity_id in the schema, but it is the unique id; -1 means a block container.
                let entity = (p.runtime_entity_id != -1).then_some(p.runtime_entity_id);
                tracing::debug!(window = ?p.window_type, id = ?p.window_id, "container open");
                let title = entity.is_none().then(|| self.title_at(&p.coordinates)).flatten();
                self.opened(Container {
                    window_id: p.window_id.to_raw() as i32,
                    window_type: p.window_type,
                    position: entity.is_none().then_some(p.coordinates),
                    entity,
                    title,
                    slots: Vec::new(),
                });
            }
            ContainerClose::ID => {
                let p: ContainerClose = packet.decode()?;
                tracing::debug!(id = ?p.window_id, server = p.server, "container close");
                let id = p.window_id.to_raw() as i32;
                // A late close for a reused id must not wipe a newer window; -1 (WindowID::None) names none, so it closes any.
                if self.open.as_ref().is_some_and(|c| c.window_id == id || id == -1) {
                    self.open = None;
                    self.data.clear();
                }
            }
            ContainerSetData::ID => {
                let p: ContainerSetData = packet.decode()?;
                if self.open_window(p.window_id.to_raw() as i32).is_some() {
                    self.data.insert(p.property, p.value);
                }
            }
            // The trade screen opens with UpdateTrade itself; a resend for the open window changes nothing here.
            UpdateTrade::ID => {
                let p: UpdateTrade = packet.decode()?;
                let window_id = p.window_id.to_raw() as i32;
                if self.open.as_ref().is_none_or(|c| c.window_id != window_id) {
                    self.opened(Container {
                        window_id,
                        window_type: p.window_type,
                        position: None,
                        entity: Some(p.entity_unique_id as i64),
                        title: (!p.display_name.is_empty()).then_some(p.display_name),
                        slots: Vec::new(),
                    });
                }
            }
            InventoryContent::ID => {
                let p: InventoryContent = packet.decode()?;
                if let Some(open) = self.open_window(p.window_id.to_raw() as i32) {
                    open.slots = p.input.into_iter().map(ItemStack::from).collect();
                    self.contents += 1;
                }
            }
            InventorySlot::ID => {
                let p: InventorySlot = packet.decode()?;
                if let Some(open) = self.open_window(p.window_id.to_raw() as i32) {
                    let slot = p.slot as usize;
                    if open.slots.len() <= slot {
                        open.slots.resize(slot + 1, ItemStack::default());
                    }
                    open.slots[slot] = p.item.into();
                }
            }
            ItemStackResponse::ID => {
                for (kind, slot) in successful_slots(packet.decode()?) {
                    if let Some(dst) = self.response_target(kind, slot.slot) {
                        dst.apply_response(&slot);
                    }
                }
            }
            BlockEntityData::ID => {
                let (position, nbt) = super::block_entity_parts(packet)?;
                self.block_entity(&position, nbt)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// A block entity's update: remembers its `CustomName` for titling windows.
    pub(crate) fn block_entity(&mut self, position: &BlockCoordinates, nbt: &[u8]) -> Result<(), DecodeError> {
        if let Some(name) = root_string::<Network>(nbt, "CustomName")? {
            self.name_block(position, &name);
        }
        Ok(())
    }

    /// The open container's slot an `ItemStackResponse` entry of container `kind` refers to.
    pub(crate) fn response_target(&mut self, kind: ContainerSlotType, slot: u8) -> Option<&mut ItemStack> {
        let open = self.open.as_mut()?;
        if !is_container_slot(kind) {
            return None;
        }
        open.slots.get_mut(usize::from(slot))
    }

    fn opened(&mut self, container: Container) {
        self.last_window_id = Some(container.window_id);
        self.data.clear();
        self.open = Some(container);
    }

    fn open_window(&mut self, window: i32) -> Option<&mut Container> {
        self.open.as_mut().filter(|c| c.window_id == window && !Inventory::is_player_window(window))
    }

    fn title_at(&self, pos: &BlockCoordinates) -> Option<String> {
        let key = [pos.x, pos.y, pos.z];
        self.titles.iter().rev().find(|(at, _)| *at == key).map(|(_, name)| name.clone())
    }

    /// Remembers a block entity's name; also titles the open window if it is that block.
    fn name_block(&mut self, pos: &BlockCoordinates, name: &str) {
        if let Some(open) = self.open.as_mut().filter(|c| c.position.as_ref() == Some(pos)) {
            open.title = Some(name.to_owned());
        }
        let key = [pos.x, pos.y, pos.z];
        self.titles.retain(|(at, _)| *at != key);
        if self.titles.len() == TITLES {
            self.titles.pop_front();
        }
        self.titles.push_back((key, name.to_owned()));
    }
}

/// Response slot types that index the open container's own slots (UI-backed types such as
/// crafting or anvil use UI-window offsets instead).
fn is_container_slot(kind: ContainerSlotType) -> bool {
    use ContainerSlotType as T;
    matches!(
        kind,
        T::Container
            | T::Barrel
            | T::Shulker
            | T::Crafter
            | T::FurnaceIngredient
            | T::FurnaceFuel
            | T::FurnaceOutput
            | T::BlastFurnaceIngredient
            | T::SmokerIngredient
            | T::BrewingInput
            | T::BrewingResult
            | T::BrewingFuel
    )
}

#[cfg(test)]
mod tests;
