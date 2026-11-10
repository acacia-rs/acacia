//! The Java player inventory as the bot's [`Inventory`]: network ids are Java item ids (see
//! [`ItemRegistry::java`]), metadata is the Bedrock data that tells colours apart, and a block item's
//! `block_runtime_id` is its block's default Java state.

use acacia_client::proto::nbt::{Nbt, Value};
use acacia_java_client::Inventory as JavaInventory;
use acacia_java_proto::item::ItemStack as JavaStack;
use acacia_client::proto::types::WindowType;
use acacia_world::neutral::Item;

use super::{Container, Containers, Inventory, ItemStack};
use crate::workstation::enchants::{bedrock_id, with_enchant_list};
use crate::items::ui;

/// Window 0's slots: armour from 5 (helmet first), the main rows from 9, the hotbar from 36.
const ARMOR_START: usize = 5;
const MAIN_START: usize = 9;
/// The crafting grid's first slot, in window 0 and in a crafting table's menu; slot 0 is the result.
pub(crate) const GRID_START: usize = 1;
/// `minecraft:crafting`'s menu id.
pub(crate) const CRAFTING_MENU: i32 = 12;

// TODO: enchantments; their ids are the server's synced registry order, not fixed.
/// `java` in Bedrock's terms; `enchantments` names the enchantment ids (the registry's order), which become
/// Bedrock's `ench` list.
pub(crate) fn stack(java: Option<&JavaStack>, enchantments: &[String]) -> ItemStack {
    let Some(s) = java.filter(|s| s.count > 0) else { return ItemStack::default() };
    let item = u16::try_from(s.item).ok().and_then(Item::from_java_id);
    let metadata = item.map_or(0, |i| i.bedrock().1.max(0) as u32);
    let block_runtime_id = item.and_then(Item::block).and_then(|b| b.default_state().java_id()).unwrap_or(0);
    let nbt = s.components.damage.map(|d| Nbt { name: String::new(), value: Value::Compound(vec![("Damage".into(), Value::Int(d))]) });
    let stack = ItemStack {
        network_id: s.item,
        count: u16::try_from(s.count).unwrap_or(u16::MAX),
        metadata,
        block_runtime_id,
        custom_name: s.components.custom_name_text(),
        nbt,
        ..ItemStack::default()
    };
    let named = |&(id, level): &(i32, i32)| Some((bedrock_id(enchantments.get(usize::try_from(id).ok()?)?)?, i16::try_from(level).ok()?));
    let ench: Vec<_> = s.components.enchantments.iter().filter_map(named).collect();
    if ench.is_empty() { stack } else { with_enchant_list(&stack, &ench) }
}

/// Copies every slot, the cursor and the crafting grids; the selected slot is the bot's own and is left alone.
pub(crate) fn mirror(inventory: &mut Inventory, java: &JavaInventory) {
    let stack = |s: Option<&JavaStack>| stack(s, &java.enchantments);
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
    for i in 0..4 {
        inventory.ui[usize::from(ui::CRAFTING_2X2) + i] = stack(java.slot(GRID_START + i));
    }
    let table = java.window.as_ref().filter(|w| w.menu == CRAFTING_MENU);
    for i in 0..9 {
        inventory.ui[usize::from(ui::CRAFTING_3X3) + i] = stack(table.and_then(|w| w.slots.get(GRID_START + i)?.as_ref()));
    }
    for &slot in STATION_UI {
        inventory.ui[usize::from(slot)] = ItemStack::default();
    }
    if let Some(window) = &java.window {
        for (i, &slot) in station_slots(window.menu).iter().enumerate() {
            inventory.ui[usize::from(slot)] = stack(window.slots.get(i).and_then(Option::as_ref));
        }
    }
}

/// Every Bedrock UI slot some Java station menu fills.
const STATION_UI: &[u8] = &[
    ui::ANVIL_INPUT, ui::ANVIL_MATERIAL, ui::STONECUTTER_INPUT, ui::TRADE_INGREDIENT_1, ui::TRADE_INGREDIENT_2, ui::LOOM_BANNER,
    ui::LOOM_DYE, ui::LOOM_PATTERN, ui::CARTOGRAPHY_INPUT, ui::CARTOGRAPHY_ADDITIONAL, ui::ENCHANTING_INPUT, ui::ENCHANTING_LAPIS,
    ui::GRINDSTONE_INPUT, ui::GRINDSTONE_ADDITIONAL, ui::BEACON_PAYMENT, ui::SMITHING_TEMPLATE, ui::SMITHING_INPUT, ui::SMITHING_MATERIAL,
];

/// A Java station menu's input slots, in menu order, as Bedrock's UI slots.
pub(crate) fn station_slots(menu: i32) -> &'static [u8] {
    match window_type(menu) {
        WindowType::Anvil => &[ui::ANVIL_INPUT, ui::ANVIL_MATERIAL],
        WindowType::Beacon => &[ui::BEACON_PAYMENT],
        WindowType::Enchantment => &[ui::ENCHANTING_INPUT, ui::ENCHANTING_LAPIS],
        WindowType::Grindstone => &[ui::GRINDSTONE_INPUT, ui::GRINDSTONE_ADDITIONAL],
        WindowType::Loom => &[ui::LOOM_BANNER, ui::LOOM_DYE, ui::LOOM_PATTERN],
        WindowType::Trading => &[ui::TRADE_INGREDIENT_1, ui::TRADE_INGREDIENT_2],
        WindowType::SmithingTable => &[ui::SMITHING_TEMPLATE, ui::SMITHING_INPUT, ui::SMITHING_MATERIAL],
        WindowType::Cartography => &[ui::CARTOGRAPHY_INPUT, ui::CARTOGRAPHY_ADDITIONAL],
        WindowType::Stonecutter => &[ui::STONECUTTER_INPUT],
        _ => &[],
    }
}

/// The menu slot the server puts a station's result in (right after its inputs); `None` for stations without one.
pub(crate) fn result_slot(menu: i32) -> Option<usize> {
    let inputs = station_slots(menu).len();
    (inputs > 0 && !matches!(window_type(menu), WindowType::Beacon | WindowType::Enchantment)).then_some(inputs)
}

/// The Java container screen as the bot's open container; the opener fills in the block position.
pub(crate) fn mirror_window(containers: &mut Containers, java: &JavaInventory) {
    let Some(window) = &java.window else {
        containers.open = None;
        return;
    };
    let slots = window.slots.iter().map(|s| stack(s.as_ref(), &java.enchantments)).collect();
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
    let furnace = matches!(window.menu, 10 | 14 | 22);
    containers.data = window.data.iter().map(|&(id, value)| (i32::from(bedrock_property(furnace, id)), i32::from(value))).collect();
}

/// A Java window property's Bedrock number: Bedrock's furnace counts 0 cook ticks, 1 fuel left, 2 fuel total
/// where Java's counts 2, 0, 1; Java's 3, the cook time, stays 3.
fn bedrock_property(furnace: bool, id: i16) -> i16 {
    match (furnace, id) {
        (true, 0) => 1,
        (true, 1) => 2,
        (true, 2) => 0,
        _ => id,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::breaking::{enchantment_level, EFFICIENCY};

    #[test]
    fn java_enchantments_become_bedrock_ench_entries() {
        let names: Vec<String> = ["minecraft:sweeping_edge", "minecraft:efficiency"].map(String::from).to_vec();
        let mut pickaxe = JavaStack::new(1052, 1);
        pickaxe.components.enchantments = vec![(1, 4), (0, 2), (7, 1)];
        let mirrored = stack(Some(&pickaxe), &names);
        assert_eq!((enchantment_level(&mirrored, EFFICIENCY), mirrored.enchantments().len()), (4, 1));
    }
}
