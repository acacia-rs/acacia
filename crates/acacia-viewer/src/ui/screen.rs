//! An open inventory screen as acacia-ui draws it: each slot's icon, the pick list (a
//! stonecutter's cuts, a loom's patterns, enchanting options, a trader's offers) and the recipe book.

use std::collections::HashMap;

use acacia_bot::workstation::AnvilCost;
use acacia_ui::DrawList;
use acacia_ui::inventory::{self, Bench, Contents, Pick, Slot};
use acacia_ui::lang::Lang;

use super::{Screen, Skin};
use crate::control::{Inventory, Stack};

/// Where a trade's two prices and its goods lie in the offer's button (Java's `MerchantScreen`).
const PRICE: [f32; 2] = [5.0, 30.0];
const GOODS: f32 = 68.0;

pub(super) fn draw(list: &mut DrawList, skin: &mut Skin, lang: &Lang, screen: &Screen, mouse: [f32; 2], gui: [f32; 2]) {
    let Screen { inventory, layout, picked, first, name, creative, .. } = *screen;
    let mut icons = HashMap::new();
    // A pick list's result is the picked one.
    let shown_in = |slot: Slot| match slot {
        Slot::Result if !inventory.picks.is_empty() => picked[0].and_then(|i| inventory.picks.get(i)).map(|p| &p.1),
        Slot::Result if !inventory.patterns.is_empty() => picked[0].and_then(|i| inventory.patterns.get(i)).and_then(|p| p.1.as_ref()),
        _ => stack_in(inventory, slot),
    };
    for (slot, _) in inventory::slots(layout) {
        if let Some(item) = shown_in(slot).and_then(|s| skin.item(s)) {
            icons.insert(slot, item);
        }
    }
    let cursor = inventory.cursor.as_ref().and_then(|s| skin.item(s));
    let mut picks = picks(skin, inventory);
    // A list longer than its window shows how far it is scrolled.
    let hidden = picks.len().saturating_sub(layout.picks().map_or(usize::MAX, |p| p.columns * p.rows));
    let scrolled = (hidden > 0).then(|| first as f32 / hidden as f32);
    picks.drain(..first.min(picks.len()));
    let shown = picked.map(|p| p.and_then(|i| i.checked_sub(first)));
    for stack in creative.iter().flat_map(|(stacks, ..)| *stacks) {
        picks.push(Pick { icons: skin.icon(stack).map(|icon| (icon, 1, 1.0)).into_iter().collect(), label: String::new(), script: String::new(), closed: false });
    }
    let note = inventory.cost.filter(|_| inventory.bench == Some(Bench::Anvil)).map(|cost| match cost {
        AnvilCost::Levels(levels) => (format!("Enchantment Cost: {levels}"), true),
        AnvilCost::Short(levels) => (format!("Enchantment Cost: {levels}"), false),
        AnvilCost::TooExpensive => ("Too Expensive!".to_owned(), false),
    });
    let contents = Contents { slot: &|slot| icons.get(&slot).copied(), cursor, progress: inventory.progress, picks: &picks, picked: shown, scrolled, name, experience: inventory.trade.as_ref().and_then(|t| t.progress), note: note.as_ref().map(|(text, payable)| (text.as_str(), *payable)), creative: creative.map(|(_, tab, scrolled)| (tab, scrolled)) };
    let title = match (&inventory.trade, &inventory.container) {
        _ if creative.is_some_and(|c| c.1 == inventory::SEARCH_TAB) => name.map_or_else(String::new, |(before, after)| format!("Search: {before}_{after}")),
        _ if creative.is_some() => inventory::TABS[creative.map_or(0, |c| c.1)].to_owned(),
        // MerchantScreen: "Farmer - Novice".
        (Some(trade), _) => trade.level.map_or_else(|| lang.translate(&trade.title, &[]), |level| format!("{} - {level}", lang.translate(&trade.title, &[]))),
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
    // The name of the stack under the mouse, while none is carried: a slot's, a creative item's
    // or a stonecutter's cut.
    let over = inventory::hit(layout, gui, mouse).and_then(shown_in);
    let over = over.or_else(|| creative.and_then(|(stacks, ..)| stacks.get(inventory::hit_pick(layout, gui, stacks.len(), mouse)?)));
    let over = over.or_else(|| inventory.picks.get(first + inventory::hit_pick(layout, gui, inventory.picks.len().saturating_sub(first), mouse)?).map(|p| &p.1));
    if let Some(stack) = over.filter(|_| inventory.cursor.is_none()) {
        let name = stack.title.as_ref().map_or_else(|| lang.item_name(&stack.name), |given| format!("§o{given}"));
        let lines: Vec<String> = std::iter::once(name).chain(crate::enchant_names::notes(stack)).collect();
        acacia_ui::tooltip::draw(list, &skin.theme, &lines, mouse, gui);
    }
}

/// The open screen's pick list: only one of the kinds is ever filled.
fn picks(skin: &mut Skin, inventory: &Inventory) -> Vec<Pick> {
    let mut at = |s: &Stack, x: f32| skin.icon(s).map(|icon| (icon, s.count, x));
    let mut picks: Vec<Pick> = inventory.picks.iter().map(|(_, s)| Pick { icons: at(&Stack { count: 1, ..s.clone() }, 0.0).into_iter().collect(), label: String::new(), script: String::new(), closed: false }).collect();
    picks.extend(inventory.enchants.iter().map(|(level, hint, words)| Pick { icons: Vec::new(), label: crate::enchant_names::option(*level, *hint), script: words.clone(), closed: false }));
    for offer in inventory.trade.iter().flat_map(|t| &t.offers) {
        let prices = [(Some(&offer.price), PRICE[0]), (offer.second_price.as_ref(), PRICE[1]), (Some(&offer.goods), GOODS)];
        let icons = prices.into_iter().filter_map(|(stack, x)| at(stack?, x)).collect();
        picks.push(Pick { icons, label: if offer.open { ">" } else { "x" }.into(), script: String::new(), closed: false });
    }
    picks.extend(inventory.patterns.iter().map(|(code, _)| Pick { icons: skin.pattern(code).map(|icon| (icon, 1, -1.0)).into_iter().collect(), label: String::new(), script: String::new(), closed: false }));
    if inventory.bench == Some(Bench::Beacon) {
        // Each button shows its effect's icon (the confirm its tick), or its name where the look
        // has no such image; a power the pyramid is too small for is closed.
        for (i, (label, image)) in crate::stations::BEACON.iter().zip(crate::stations::BEACON_ICONS).enumerate() {
            let icon = skin.theme.atlas.get(image).or_else(|| skin.theme.atlas.get(&acacia_ui::effects::sprite(image)));
            let label = if icon.is_some() { String::new() } else { (*label).to_owned() };
            picks.push(Pick { icons: icon.map(|icon| (icon, 1, 3.0)).into_iter().collect(), label, script: String::new(), closed: !crate::stations::beacon_allows(i, inventory.beacon) });
        }
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
