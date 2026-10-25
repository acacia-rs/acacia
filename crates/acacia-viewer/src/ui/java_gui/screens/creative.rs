//! The creative inventory over the server's creative items. Bedrock's four item tabs show as the
//! Java tabs nearest them, in Java's places, with the search and survival inventory tabs; a tab's
//! index is Bedrock's (`acacia_ui::inventory::TABS`), so the app's tab and search are shared.

use acacia_bot::items::SlotRef;
use acacia_ui::inventory::{SEARCH_TAB, SURVIVAL_TAB};
use acacia_ui::java::container::creative::{Creative, GRID, ICONS, Tab, TabType};

use super::super::items::{item_stack, java_id, plain};
use super::build::{Kit, fill};
use super::player::{effects, player_figure};
use super::{Open, Target};
use crate::creative_items::CreativeEntry;

/// Per tab: Java's tab, its row and column, its icon.
const TABS: [(&str, bool, i32, &str); 6] = [
    ("buildingBlocks", true, 0, "minecraft:brick_block"),
    ("natural", true, 2, "minecraft:grass_block"),
    ("tools", false, 0, "minecraft:diamond_pickaxe"),
    ("ingredients", false, 3, "minecraft:iron_ingot"),
    ("inventory", false, 6, "minecraft:chest"),
    ("search", true, 6, "minecraft:compass"),
];

fn tabs(kit: &Kit) -> Vec<Tab> {
    let tab = |(i, &(title, top, column, _)): (usize, &(&str, bool, i32, &str))| {
        let kind = match i {
            SURVIVAL_TAB => TabType::Inventory,
            SEARCH_TAB => TabType::Search,
            _ => TabType::Category,
        };
        let background = match kind {
            TabType::Inventory => "inventory",
            TabType::Search => "item_search",
            _ => "items",
        };
        Tab {
            kind,
            top,
            column,
            right: column == 6,
            show_title: kind != TabType::Inventory,
            can_scroll: kind != TabType::Inventory,
            background: format!("textures/gui/container/creative_inventory/tab_{background}.png"),
            title: kit.tr(&format!("itemGroup.{title}")),
        }
    };
    TABS.iter().enumerate().map(tab).collect()
}

/// Whether the entry's name or id holds the search's words.
fn found(kit: &Kit, entry: &CreativeEntry, sought: &str) -> bool {
    let stack = item_stack(&entry.stack, kit.items, kit.lang, 0);
    let name = acacia_ui::java::tooltip::item::hover_name(&stack, kit.env.lang, kit.env.game).plain().to_lowercase();
    name.contains(sought) || java_id(&entry.stack, kit.items).contains(sought)
}

/// The tab's entries, in the server's order.
pub(super) fn listed<'a>(kit: &Kit<'a>, tab: usize) -> Vec<&'a CreativeEntry> {
    let entries = kit.view.entries;
    let sought = kit.view.name.0.trim().to_lowercase();
    entries.iter().filter(|e| if tab == SEARCH_TAB { sought.is_empty() || found(kit, e, &sought) } else { e.tab == tab }).collect()
}

/// `inventory_slot`'s targets on the survival inventory tab.
fn inventory_targets() -> Vec<Target> {
    (0..47)
        .map(|i: u8| match i {
            1..=4 => Target::Slot(SlotRef::Ui(acacia_bot::items::ui::CRAFTING_2X2 + i - 1)),
            5..=8 => Target::Slot(SlotRef::Armor(i - 5)),
            9..=35 => Target::Slot(SlotRef::Main(i)),
            36..=44 => Target::Slot(SlotRef::Main(i - 36)),
            45 => Target::Slot(SlotRef::Offhand),
            46 => Target::Destroy,
            _ => Target::None,
        })
        .collect()
}

pub(super) fn creative(kit: &mut Kit) -> (Open, Vec<Target>) {
    let [w, h] = kit.size();
    let selected = kit.view.creative.unwrap_or(0).min(TABS.len() - 1);
    let mut s = Creative::new(w, h, tabs(kit), selected, player_figure());
    for (i, &(.., icon)) in TABS.iter().enumerate() {
        kit.keyed(&plain(icon), ICONS + i as i32);
    }
    let targets = if selected == SURVIVAL_TAB {
        inventory_targets()
    } else {
        let listed = listed(kit, selected);
        s.set_items(listed.len(), |_| None, kit.state.scroll);
        let rows = (listed.len() as i32 + 8) / 9 - 5;
        let row = (((s.scroll * rows as f32) as f64 + 0.5) as i32).max(0) as usize;
        let grid = (0..GRID).map(|i| listed.get(i + row * 9).map_or(Target::None, |e| Target::Creative(e.id)));
        grid.chain((0..9).map(|i| Target::Slot(SlotRef::Main(i)))).collect()
    };
    fill(kit, &mut s.base, &targets);
    if selected == SEARCH_TAB {
        let (text, caret) = (kit.view.name.0.as_str(), kit.view.name.1);
        let font = kit.env.font;
        s.search.set_value(font, text);
        s.search.set_cursor(font, caret);
        s.search.set_highlight(font, caret);
        s.search.blink_on = kit.ticks() / 6 % 2 == 0;
    }
    s.effects = effects(kit);
    (Open::Creative(s), targets)
}
