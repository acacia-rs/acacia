use std::collections::HashMap;

use acacia_client::proto::packets::{
    ContainerClose, ContainerOpen, ContainerSetData, InventoryContent, InventorySlot, ItemStackResponse, UpdateTrade,
};
use acacia_client::proto::types::{BlockCoordinates, ContainerSlotType, WindowType};
use acacia_client::proto::{DecodeError, Packet, RawPacket};

use super::inventory::successful_slots;
use super::{Inventory, ItemStack, Me};

/// One open container window (chest, shop GUI, furnace, ...).
#[derive(Debug, Clone, PartialEq)]
pub struct Container {
    pub window_id: i32,
    pub window_type: WindowType,
    /// The block that was opened, unless it is an entity container.
    pub position: Option<BlockCoordinates>,
    /// Unique id of the entity that was opened (horse, chest minecart, ...).
    pub entity: Option<i64>,
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
    ];

    pub fn apply(&mut self, packet: &RawPacket, _me: &Me) -> Result<(), DecodeError> {
        match packet.id {
            ContainerOpen::ID => {
                let p: ContainerOpen = packet.decode()?;
                // Named runtime_entity_id in the schema, but it is the unique id; -1 means a block container.
                let entity = (p.runtime_entity_id != -1).then_some(p.runtime_entity_id);
                self.data.clear();
                self.open = Some(Container {
                    window_id: p.window_id.to_raw() as i32,
                    window_type: p.window_type,
                    position: entity.is_none().then_some(p.coordinates),
                    entity,
                    slots: Vec::new(),
                });
            }
            ContainerClose::ID => {
                packet.decode::<ContainerClose>()?;
                self.open = None;
                self.data.clear();
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
                    self.open = Some(Container {
                        window_id,
                        window_type: p.window_type,
                        position: None,
                        entity: Some(p.entity_unique_id as i64),
                        slots: Vec::new(),
                    });
                }
            }
            InventoryContent::ID => {
                let p: InventoryContent = packet.decode()?;
                if let Some(open) = self.open_window(p.window_id.to_raw() as i32) {
                    open.slots = p.input.into_iter().map(ItemStack::from).collect();
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
            _ => {}
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

    fn open_window(&mut self, window: i32) -> Option<&mut Container> {
        self.open.as_mut().filter(|c| c.window_id == window && !Inventory::is_player_window(window))
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
mod tests {
    use acacia_client::proto::types::{WindowID, WindowIDVarint};

    use super::*;
    use crate::state::queries::test_support::{content, empty_item, fixtures, item, named_item, raw, response, slot_packet};

    const ME: Me = Me { runtime_entity_id: 7, unique_entity_id: -7 };
    const CHEST: WindowIDVarint = WindowIDVarint::First;

    fn open_chest(containers: &mut Containers) {
        let open = ContainerOpen {
            window_id: WindowID::First,
            window_type: WindowType::Container,
            coordinates: BlockCoordinates { x: 10, y: 64, z: -3 },
            runtime_entity_id: -1,
        };
        containers.apply(&raw(&open), &ME).unwrap();
    }

    #[test]
    fn open_fill_update_close() {
        let mut containers = Containers::default();
        open_chest(&mut containers);
        let open = containers.open.as_ref().unwrap();
        assert_eq!((open.window_id, open.window_type), (1, WindowType::Container));
        assert_eq!(open.position, Some(BlockCoordinates { x: 10, y: 64, z: -3 }));
        assert_eq!(open.entity, None);

        let mut items = vec![empty_item(); 27];
        items[13] = named_item(304, 1, "§aBuy Diamond", &["$100"]);
        containers.apply(&raw(&content(CHEST, items)), &ME).unwrap();
        containers.apply(&raw(&content(WindowIDVarint::Inventory, vec![item(1, 1, 1)])), &ME).unwrap();
        containers.apply(&raw(&slot_packet(CHEST, 2, item(5, 8, 50))), &ME).unwrap();
        containers.apply(&raw(&response(true, ContainerSlotType::Container, 2, 3, 51, "")), &ME).unwrap();
        containers.apply(&raw(&response(true, ContainerSlotType::Hotbar, 13, 0, 0, "")), &ME).unwrap();

        let open = containers.open.as_ref().unwrap();
        assert_eq!(open.slots.len(), 27);
        assert_eq!(open.slot(13).unwrap().custom_name.as_deref(), Some("§aBuy Diamond"));
        assert_eq!((open.slot(2).unwrap().count, open.slot(2).unwrap().stack_network_id), (3, Some(51)));
        assert_eq!(open.iter().count(), 2);

        containers.apply(&raw(&ContainerClose { window_id: WindowID::First, window_type: WindowType::Container, server: true }), &ME).unwrap();
        assert!(containers.open.is_none());
    }

    #[test]
    fn entity_container_and_slot_growth() {
        let mut containers = Containers::default();
        let open = ContainerOpen {
            window_id: WindowID::First,
            window_type: WindowType::Horse,
            coordinates: BlockCoordinates { x: 0, y: 0, z: 0 },
            runtime_entity_id: 42,
        };
        containers.apply(&raw(&open), &ME).unwrap();
        containers.apply(&raw(&slot_packet(CHEST, 4, item(9, 1, 2))), &ME).unwrap();
        let open = containers.open.as_ref().unwrap();
        assert_eq!((open.entity, open.position.clone()), (Some(42), None));
        assert_eq!(open.slots.len(), 5);
        assert_eq!(open.slot(4).unwrap().network_id, 9);
    }

    #[test]
    fn decodes_fixtures() {
        let mut containers = Containers::default();
        for packet in fixtures::<ContainerOpen>().into_iter().chain(fixtures::<InventoryContent>()).chain(fixtures::<ContainerClose>()) {
            containers.apply(&packet, &ME).unwrap();
        }
        assert!(containers.open.is_none());
    }
}
