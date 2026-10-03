use acacia_client::proto::types::{ContainerSlotType, FullContainerName, StackRequestSlotInfo, WindowType};

use super::ui;
use crate::state::{Container, GameState, Inventory, ItemStack};

/// A slot the bot can move items from or to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotRef {
    /// The player's main inventory, 0-35: 0-8 are the hotbar, 9-35 the inventory.
    Main(u8),
    /// Helmet, chestplate, leggings, boots (0-3).
    Armor(u8),
    Offhand,
    /// The stack held by the mouse cursor.
    Cursor,
    /// A slot of the open container (chest, barrel, furnace, server GUI, ...).
    Container(u8),
    /// A workstation slot of the player's UI window by its offset (crafting grid, anvil,
    /// enchanting table, created output, ...; see [`crate::items::ui`]).
    Ui(u8),
}

impl SlotRef {
    /// Where craft results appear before they are moved out.
    pub const CREATED_OUTPUT: SlotRef = SlotRef::Ui(ui::CREATED_OUTPUT);

    pub fn hotbar(slot: u8) -> Self {
        SlotRef::Main(slot)
    }

    /// One of the player's own slots: main inventory, armor, offhand or cursor.
    pub fn is_player(self) -> bool {
        !matches!(self, SlotRef::Container(_) | SlotRef::Ui(_))
    }

    /// The tracked stack in this slot; `None` if the slot does not exist (or no container is open).
    pub fn stack(self, state: &GameState) -> Option<&ItemStack> {
        let inv = &state.inventory;
        match self {
            SlotRef::Main(i) => inv.main.get(usize::from(i)),
            SlotRef::Armor(i) => inv.armor.get(usize::from(i)),
            SlotRef::Offhand => Some(&inv.offhand),
            SlotRef::Cursor => Some(inv.cursor()),
            SlotRef::Container(i) => open_container(state)?.slots.get(usize::from(i)),
            SlotRef::Ui(i) => ui::slot_type(i).and_then(|_| inv.ui.get(usize::from(i))),
        }
    }

    pub(crate) fn stack_mut(self, state: &mut GameState) -> Option<&mut ItemStack> {
        let inv = &mut state.inventory;
        match self {
            SlotRef::Main(i) => inv.main.get_mut(usize::from(i)),
            SlotRef::Armor(i) => inv.armor.get_mut(usize::from(i)),
            SlotRef::Offhand => Some(&mut inv.offhand),
            SlotRef::Cursor => Some(&mut inv.ui[0]),
            SlotRef::Ui(i) => ui::slot_type(i).and_then(|_| inv.ui.get_mut(usize::from(i))),
            SlotRef::Container(i) => state.containers.open.as_mut().filter(|c| !is_own_screen(c))?.slots.get_mut(usize::from(i)),
        }
    }
}

/// The open non-player container, if any (opening the own inventory also shows up as open).
pub(crate) fn open_container(state: &GameState) -> Option<&Container> {
    state.containers.open.as_ref().filter(|c| !is_own_screen(c))
}

/// The own inventory screen: BDS opens it as the next window id (not 0) with type `Inventory`.
pub(crate) fn is_own_screen(container: &Container) -> bool {
    Inventory::is_player_window(container.window_id) || container.window_type == WindowType::Inventory
}

/// How a slot is named on the wire for the current screen. See [`SlotRef::wire`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Screen {
    pub window_type: WindowType,
    pub block: BlockKind,
}

/// Barrels and shulker boxes open as `WindowType::Container` like chests but use their own slot
/// type; only the block tells them apart, so this needs terrain tracking (physics bots).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum BlockKind {
    #[default]
    Other,
    Barrel,
    ShulkerBox,
}

impl BlockKind {
    pub fn from_name(name: &str) -> Self {
        if name == "minecraft:barrel" {
            BlockKind::Barrel
        } else if name.ends_with("shulker_box") {
            BlockKind::ShulkerBox
        } else {
            BlockKind::Other
        }
    }
}

impl Screen {
    pub fn of(state: &GameState, block: BlockKind) -> Self {
        let open = open_container(state);
        Screen { window_type: open.map_or(WindowType::Inventory, |c| c.window_type), block }
    }
}

impl SlotRef {
    /// Container name and slot index as sent in `ItemStackRequest`s, as vanilla names them; `auto`
    /// marks an automatic move (`request::auto_move`).
    pub(crate) fn wire(self, screen: &Screen, auto: bool) -> (ContainerSlotType, u8) {
        use ContainerSlotType as T;
        match self {
            // In the own inventory screen too: BDS rejects a recipe-book Consume from `Hotbar` (status 27).
            SlotRef::Main(i) if auto => (T::HotbarAndInventory, i),
            SlotRef::Main(i) if i < Inventory::HOTBAR_SLOTS => (T::Hotbar, i),
            SlotRef::Main(i) => (T::Inventory, i),
            SlotRef::Armor(i) => (T::Armor, i),
            // Geyser (and vanilla) address the offhand as slot 1.
            SlotRef::Offhand => (T::Offhand, 1),
            SlotRef::Cursor => (T::Cursor, 0),
            SlotRef::Container(i) => (container_slot_type(screen, i), i),
            SlotRef::Ui(i) => (ui::slot_type(i).unwrap_or(ContainerSlotType::Cursor), i),
        }
    }

    /// The request's view of this slot: container name, slot and the id of the stack it holds.
    pub(crate) fn slot_info(self, screen: &Screen, auto: bool, stack: &ItemStack) -> StackRequestSlotInfo {
        let stack_id = if stack.is_empty() { 0 } else { stack.stack_network_id.unwrap_or(0) };
        self.slot_info_with_id(screen, auto, stack_id)
    }

    pub(crate) fn slot_info_with_id(self, screen: &Screen, auto: bool, stack_id: i32) -> StackRequestSlotInfo {
        let (kind, slot) = self.wire(screen, auto);
        StackRequestSlotInfo { slot_type: FullContainerName { container_id: kind, dynamic_container_id: None }, slot, stack_id }
    }
}

fn container_slot_type(screen: &Screen, slot: u8) -> ContainerSlotType {
    use ContainerSlotType as T;
    use WindowType as W;
    let furnace = |ingredient| match slot {
        0 => ingredient,
        1 => T::FurnaceFuel,
        _ => T::FurnaceOutput,
    };
    match screen.window_type {
        W::Furnace => furnace(T::FurnaceIngredient),
        W::BlastFurnace => furnace(T::BlastFurnaceIngredient),
        W::Smoker => furnace(T::SmokerIngredient),
        W::BrewingStand => match slot {
            0 => T::BrewingInput,
            4 => T::BrewingFuel,
            _ => T::BrewingResult,
        },
        W::Crafter => T::Crafter,
        W::Container => match screen.block {
            BlockKind::Barrel => T::Barrel,
            BlockKind::ShulkerBox => T::Shulker,
            BlockKind::Other => T::Container,
        },
        _ => T::Container,
    }
}
