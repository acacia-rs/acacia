use acacia_client::proto::packets::{InventoryContent, InventorySlot, InventoryTransaction, ItemStackResponse, MobEquipment, PlayerHotbar};
use acacia_client::proto::types::{
    ContainerSlotType, ItemStackResponsesItemContainersItemSlotsItem as ResponseSlot, ItemStackResponsesItemStatus,
    TransactionActionsItemSourceType, WindowID,
};
use acacia_client::proto::{DecodeError, Packet, RawPacket};

use super::Me;

mod stack;

pub use stack::ItemStack;
use stack::EMPTY;

/// Slot updates from the successful responses in an `ItemStackResponse`, with their container.
pub(super) fn successful_slots(packet: ItemStackResponse) -> impl Iterator<Item = (ContainerSlotType, ResponseSlot)> {
    packet
        .responses
        .into_iter()
        .filter(|r| r.status == ItemStackResponsesItemStatus::Ok)
        .flat_map(|r| r.containers.unwrap_or_default())
        .flat_map(|c| {
            let kind = c.slot_type.container_id;
            c.slots.into_iter().map(move |s| (kind, s))
        })
}

/// The local player's own inventory windows.
#[derive(Debug)]
pub struct Inventory {
    /// 36 slots; 0-8 are the hotbar.
    pub main: Vec<ItemStack>,
    /// Helmet, chestplate, leggings, boots.
    pub armor: Vec<ItemStack>,
    pub offhand: ItemStack,
    /// The UI window (124): slot 0 is the cursor, the rest hold workstation inputs at vanilla's
    /// offsets (crafting grid 28-40, created output 50, anvil 1-2, ...; see `crate::items::ui`).
    pub ui: Vec<ItemStack>,
    pub selected_hotbar_slot: u8,
}

impl Default for Inventory {
    fn default() -> Self {
        Inventory {
            main: vec![ItemStack::default(); Self::MAIN_SLOTS],
            armor: vec![ItemStack::default(); Self::ARMOR_SLOTS],
            offhand: ItemStack::default(),
            ui: vec![ItemStack::default(); Self::UI_SLOTS],
            selected_hotbar_slot: 0,
        }
    }
}

impl Inventory {
    pub const PACKETS: &'static [u32] =
        &[InventoryContent::ID, InventorySlot::ID, MobEquipment::ID, PlayerHotbar::ID, ItemStackResponse::ID, InventoryTransaction::ID];

    pub const WINDOW_INVENTORY: i32 = 0;
    pub const WINDOW_OFFHAND: i32 = 119;
    pub const WINDOW_ARMOR: i32 = 120;
    pub const WINDOW_UI: i32 = 124;
    pub const MAIN_SLOTS: usize = 36;
    pub const ARMOR_SLOTS: usize = 4;
    pub const HOTBAR_SLOTS: u8 = 9;
    pub const UI_SLOTS: usize = 54;

    pub fn is_player_window(window: i32) -> bool {
        matches!(window, Self::WINDOW_INVENTORY | Self::WINDOW_OFFHAND | Self::WINDOW_ARMOR | Self::WINDOW_UI)
    }

    pub fn apply(&mut self, packet: &RawPacket, me: &Me) -> Result<(), DecodeError> {
        match packet.id {
            InventoryContent::ID => {
                let p: InventoryContent = packet.decode()?;
                tracing::trace!(window = ?p.window_id, "inventory content");
                if let Some(window) = self.window_mut(p.window_id.to_raw() as i32) {
                    for (dst, src) in window.iter_mut().zip(p.input) {
                        *dst = src.into();
                    }
                }
            }
            InventorySlot::ID => {
                let p: InventorySlot = packet.decode()?;
                tracing::trace!(window = ?p.window_id, slot = p.slot, id = p.item.network_id, count = p.item.count, "inventory slot");
                if let Some(dst) = self.window_mut(p.window_id.to_raw() as i32).and_then(|w| w.get_mut(p.slot as usize)) {
                    *dst = p.item.into();
                }
            }
            MobEquipment::ID => {
                let p: MobEquipment = packet.decode()?;
                if p.runtime_entity_id == me.runtime_entity_id && p.window_id == WindowID::Inventory {
                    self.select(p.selected_slot);
                }
            }
            PlayerHotbar::ID => {
                let p: PlayerHotbar = packet.decode()?;
                if p.select_slot && p.window_id == WindowID::Inventory {
                    self.select(u8::try_from(p.selected_slot).unwrap_or(u8::MAX));
                }
            }
            ItemStackResponse::ID => {
                for (kind, slot) in successful_slots(packet.decode()?) {
                    if let Some(dst) = self.response_target(kind, slot.slot) {
                        dst.apply_response(&slot);
                    }
                }
            }
            // BDS confirms a book edit with an InventoryContent of the old book, then this transaction.
            // Pickups arrive this way too (WorldInteraction source, before TakeItemEntity): nothing to predict.
            InventoryTransaction::ID => {
                let p: InventoryTransaction = packet.decode()?;
                tracing::trace!(actions = ?p.transaction.actions.iter().map(|a| (a.source_type, a.window_id, a.slot, a.new_item.count)).collect::<Vec<_>>(), "inventory transaction");
                let changes = p.transaction.actions.into_iter().filter(|a| a.source_type == TransactionActionsItemSourceType::Container);
                for a in changes {
                    if let Some(dst) = a.window_id.and_then(|w| self.window_mut(i32::from(w))).and_then(|w| w.get_mut(a.slot as usize)) {
                        *dst = a.new_item.into();
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// The stack held by the mouse cursor (UI slot 0).
    pub fn cursor(&self) -> &ItemStack {
        &self.ui[0]
    }

    /// The slots of one of the player's windows (see the `WINDOW_*` ids).
    pub fn window(&self, window: i32) -> Option<&[ItemStack]> {
        match window {
            Self::WINDOW_INVENTORY => Some(&self.main),
            Self::WINDOW_OFFHAND => Some(std::slice::from_ref(&self.offhand)),
            Self::WINDOW_ARMOR => Some(&self.armor),
            Self::WINDOW_UI => Some(&self.ui),
            _ => None,
        }
    }

    pub fn slot(&self, window: i32, slot: usize) -> Option<&ItemStack> {
        self.window(window)?.get(slot)
    }

    pub fn held(&self) -> &ItemStack {
        self.main.get(usize::from(self.selected_hotbar_slot)).unwrap_or(&EMPTY)
    }

    /// Every non-empty stack as `(window, slot, stack)`: main, armor, offhand, then the UI window
    /// (cursor and workstation inputs).
    pub fn iter(&self) -> impl Iterator<Item = (i32, usize, &ItemStack)> {
        [Self::WINDOW_INVENTORY, Self::WINDOW_ARMOR, Self::WINDOW_OFFHAND, Self::WINDOW_UI]
            .into_iter()
            .flat_map(|w| self.window(w).unwrap_or_default().iter().enumerate().map(move |(i, s)| (w, i, s)))
            .filter(|(_, _, s)| !s.is_empty())
    }

    pub fn find(&self, pred: impl Fn(&ItemStack) -> bool) -> Option<(i32, usize, &ItemStack)> {
        self.iter().find(|(_, _, s)| pred(s))
    }

    pub fn count_of(&self, network_id: i32) -> u32 {
        self.iter().filter(|(_, _, s)| s.network_id == network_id).map(|(_, _, s)| u32::from(s.count)).sum()
    }

    /// Also used by `Bot::select_hotbar`: servers don't echo the client's own selection back.
    pub(crate) fn select(&mut self, slot: u8) {
        if slot < Self::HOTBAR_SLOTS {
            self.selected_hotbar_slot = slot;
        }
    }

    fn window_mut(&mut self, window: i32) -> Option<&mut [ItemStack]> {
        match window {
            Self::WINDOW_INVENTORY => Some(&mut self.main),
            Self::WINDOW_OFFHAND => Some(std::slice::from_mut(&mut self.offhand)),
            Self::WINDOW_ARMOR => Some(&mut self.armor),
            Self::WINDOW_UI => Some(&mut self.ui),
            _ => None,
        }
    }

    /// The slot an `ItemStackResponse` entry of container `kind` refers to, if it is the player's.
    pub(crate) fn response_target(&mut self, kind: ContainerSlotType, slot: u8) -> Option<&mut ItemStack> {
        match kind {
            ContainerSlotType::Hotbar | ContainerSlotType::Inventory | ContainerSlotType::HotbarAndInventory => {
                self.main.get_mut(usize::from(slot))
            }
            ContainerSlotType::Armor => self.armor.get_mut(usize::from(slot)),
            ContainerSlotType::Offhand => Some(&mut self.offhand),
            ContainerSlotType::Cursor => Some(&mut self.ui[0]),
            kind if is_ui_slot(kind) => self.ui.get_mut(usize::from(slot)),
            _ => None,
        }
    }
}

/// Request/response slot types that address the UI window by its absolute offset.
fn is_ui_slot(kind: ContainerSlotType) -> bool {
    use ContainerSlotType as T;
    matches!(
        kind,
        T::AnvilInput
            | T::AnvilMaterial
            | T::SmithingTableInput
            | T::SmithingTableMaterial
            | T::SmithingTableTemplate
            | T::BeaconPayment
            | T::CraftingInput
            | T::CreativeOutput
            | T::EnchantingInput
            | T::EnchantingLapis
            | T::TradeIngredient1
            | T::TradeIngredient2
            | T::Trade2Ingredient1
            | T::Trade2Ingredient2
            | T::LoomInput
            | T::LoomDye
            | T::LoomMaterial
            | T::GrindstoneInput
            | T::GrindstoneAdditional
            | T::StonecutterInput
            | T::CartographyInput
            | T::CartographyAdditional
            | T::MatreduceInput
            | T::CompcreateInput
            | T::LabtableInput
    )
}

#[cfg(test)]
mod tests;
