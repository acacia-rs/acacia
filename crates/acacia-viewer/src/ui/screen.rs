//! An open inventory screen as acacia-ui draws it: each slot's icon, the pick list (a
//! stonecutter's cuts, enchanting options, a trader's offers) and the recipe book.

use std::collections::HashMap;

use acacia_ui::DrawList;
use acacia_ui::inventory::{self, Contents, Layout, Pick, Slot};
use acacia_ui::lang::Lang;

use super::Skin;
use crate::control::{Inventory, Stack};

/// Where a trade's two prices and its goods lie in the offer's button (Java's `MerchantScreen`).
const PRICE: [f32; 2] = [5.0, 30.0];
const GOODS: f32 = 68.0;

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(list: &mut DrawList, skin: &mut Skin, lang: &Lang, inventory: &Inventory, layout: Layout, picked: Option<usize>, mouse: [f32; 2], gui: [f32; 2]) {
    let mut icons = HashMap::new();
    for (slot, _) in inventory::slots(layout) {
        // A pick list's result is the picked one.
        let shown = match slot {
            Slot::Result if !inventory.picks.is_empty() => picked.and_then(|i| inventory.picks.get(i)).map(|p| &p.1),
            _ => stack_in(inventory, slot),
        };
        if let Some(item) = shown.and_then(|s| skin.item(s)) {
            icons.insert(slot, item);
        }
    }
    let cursor = inventory.cursor.as_ref().and_then(|s| skin.item(s));
    let picks = picks(skin, inventory);
    let contents = Contents { slot: &|slot| icons.get(&slot).copied(), cursor, progress: inventory.progress, picks: &picks, picked };
    let title = match (&inventory.trade, &inventory.container) {
        (Some(trade), _) => lang.translate(&trade.title, &[]),
        (_, Some(container)) => container.title.clone(),
        _ => String::new(),
    };
    inventory::draw(list, &skin.theme, layout, &title, &contents, mouse, gui);
    if layout.has_book() {
        // Every result keeps its cell (clicks index the same list); a missing icon draws blank.
        let blank = |s: &Stack, skin: &Skin| acacia_ui::hud::Item { icon: skin.theme.atlas.white(), count: s.count, glint: false };
        let results: Vec<_> = inventory.craftable.iter().map(|s| skin.item(s).unwrap_or_else(|| blank(s, skin))).collect();
        acacia_ui::recipes::draw(list, &skin.theme, layout, &results, mouse, gui);
    }
}

/// The open screen's pick list: only one of the three kinds is ever filled.
fn picks(skin: &mut Skin, inventory: &Inventory) -> Vec<Pick> {
    let mut at = |s: &Stack, x: f32| skin.icon(&s.name, s.aux, s.block).map(|icon| (icon, s.count, x));
    let mut picks: Vec<Pick> = inventory.picks.iter().map(|(_, s)| Pick { icons: at(&Stack { count: 1, ..s.clone() }, 0.0).into_iter().collect(), label: String::new() }).collect();
    picks.extend(inventory.enchants.iter().enumerate().map(|(i, level)| Pick { icons: Vec::new(), label: format!("{}  Level {level}", i + 1) }));
    for offer in inventory.trade.iter().flat_map(|t| &t.offers) {
        let prices = [(Some(&offer.price), PRICE[0]), (offer.second_price.as_ref(), PRICE[1]), (Some(&offer.goods), GOODS)];
        let icons = prices.into_iter().filter_map(|(stack, x)| at(stack?, x)).collect();
        picks.push(Pick { icons, label: if offer.open { ">" } else { "x" }.into() });
    }
    picks
}

fn stack_in(inventory: &Inventory, slot: Slot) -> Option<&Stack> {
    match slot {
        Slot::Main(i) => inventory.main.get(usize::from(i))?.as_ref(),
        Slot::Armor(i) => inventory.armor.get(usize::from(i))?.as_ref(),
        Slot::Offhand => inventory.offhand.as_ref(),
        Slot::Container(i) => inventory.container.as_ref()?.slots.get(usize::from(i))?.as_ref(),
        Slot::Ui(i) => inventory.ui.get(usize::from(i))?.as_ref(),
        Slot::Result => inventory.crafted.as_ref(),
    }
}
