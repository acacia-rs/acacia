//! The Java player inventory as the bot's [`Inventory`]: network ids are Java item ids (see
//! [`ItemRegistry::java`]), metadata is the Bedrock data that tells colours apart, and a block item's
//! `block_runtime_id` is its block's default Java state.

use acacia_client::proto::nbt::{Nbt, Value};
use acacia_java_client::Inventory as JavaInventory;
use acacia_java_proto::item::ItemStack as JavaStack;
use acacia_world::neutral::Item;

use super::{Inventory, ItemStack};

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

/// Copies every slot; the selected slot is the bot's own and is left alone.
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
}
