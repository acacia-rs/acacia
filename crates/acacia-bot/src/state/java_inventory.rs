//! The Java player inventory as the bot's [`Inventory`]: network ids are Java item ids (see
//! [`ItemRegistry::java`]), metadata is the Bedrock data that tells colours apart, and a block item's
//! `block_runtime_id` is its block's default Java state.

use acacia_client::proto::nbt::{Nbt, Value};
use acacia_java_client::Inventory as JavaInventory;
use acacia_java_proto::item::ItemStack as JavaStack;
use acacia_client::proto::types::WindowType;
use acacia_world::neutral::Item;

use super::{Container, Containers, Inventory, ItemStack};

/// Window 0's slots: armour from 5 (helmet first), the main rows from 9, the hotbar from 36.
const ARMOR_START: usize = 5;
const MAIN_START: usize = 9;

// TODO: enchantments; their ids are the server's synced registry order, not fixed.
pub(crate) fn stack(java: Option<&JavaStack>) -> ItemStack {
    let Some(s) = java.filter(|s| s.count > 0) else { return ItemStack::default() };
    let item = u16::try_from(s.item).ok().and_then(Item::from_java_id);
    let metadata = item.map_or(0, |i| i.bedrock().1.max(0) as u32);
    let block_runtime_id = item.and_then(Item::block).and_then(|b| b.default_state().java_id()).unwrap_or(0);
    let nbt = s.components.damage.map(|d| Nbt { name: String::new(), value: Value::Compound(vec![("Damage".into(), Value::Int(d))]) });
    ItemStack {
        network_id: s.item,
        count: u16::try_from(s.count).unwrap_or(u16::MAX),
        metadata,
        block_runtime_id,
        custom_name: s.components.custom_name_text(),
        nbt,
        ..ItemStack::default()
    }
}

/// Copies every slot and the cursor; the selected slot is the bot's own and is left alone.
pub(crate) fn mirror(inventory: &mut Inventory, java: &JavaInventory) {
    for i in 0..Inventory::HOTBAR_SLOTS as usize {
        inventory.main[i] = stack(java.hotbar(i));
    }
    for i in Inventory::HOTBAR_SLOTS as usize..Inventory::MAIN_SLOTS {
        inventory.main[i] = stack(java.slot(MAIN_START + i - Inventory::HOTBAR_SLOTS as usize));
    }
    for (i, armor) in inventory.armor.iter_mut().enumerate() {
        *armor = stack(java.slot(ARMOR_START + i));
    }
    inventory.offhand = stack(java.offhand());
    inventory.ui[0] = stack(java.cursor.as_ref());
}

/// The Java container screen as the bot's open container; the opener fills in the block position.
pub(crate) fn mirror_window(containers: &mut Containers, java: &JavaInventory) {
    let Some(window) = &java.window else {
        containers.open = None;
        return;
    };
    let slots = window.slots.iter().map(|s| stack(s.as_ref())).collect();
    containers.contents += 1;
    match containers.open.as_mut().filter(|c| c.window_id == window.id) {
        Some(open) => open.slots = slots,
        None => containers.opened(Container {
            window_id: window.id,
            window_type: window_type(window.menu),
            position: None,
            entity: None,
            title: window.title.clone(),
            slots,
        }),
    }
}

/// A `minecraft:menu` registry id (26.3) as the Bedrock window that shows the same container.
fn window_type(menu: i32) -> WindowType {
    match menu {
        0..=5 | 20 => WindowType::Container,
        6 => WindowType::Dispenser,
        7 => WindowType::Crafter,
        8 => WindowType::Anvil,
        9 => WindowType::Beacon,
        10 => WindowType::BlastFurnace,
        11 => WindowType::BrewingStand,
        12 => WindowType::Workbench,
        13 => WindowType::Enchantment,
        14 => WindowType::Furnace,
        15 => WindowType::Grindstone,
        16 => WindowType::Hopper,
        17 => WindowType::Lectern,
        18 => WindowType::Loom,
        19 => WindowType::Trading,
        21 => WindowType::SmithingTable,
        22 => WindowType::Smoker,
        23 => WindowType::Cartography,
        24 => WindowType::Stonecutter,
        _ => WindowType::None,
    }
}
