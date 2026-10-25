//! Screens with the recipe book (the player's inventory with its effects, the crafting table,
//! furnaces) and the merchant's.
//!
//! The book lists what the bot says can be crafted now, one recipe per button on one search tab;
//! its filter, pages, tab and search box are `State::book`'s. Ingredient overlays and ghost
//! recipes need the recipe set, which the viewer does not have.

use acacia_bot::items::{SlotRef, ui};
use acacia_ui::java::container::effects::Effect;
use acacia_ui::java::container::entity::Figure;
use acacia_ui::java::container::merchant::{Merchant, Offer};
use acacia_ui::java::container::recipe_book::screens::{RecipeScreen, Window};
use acacia_ui::java::container::recipe_book::{Book, Collection, Entry, Kind, Search, Tab};
use acacia_ui::java::text::Text;

use super::super::items::plain;
use super::build::{Kit, fill, player_targets};
use super::{Open, Target};
use crate::control::Stack;

/// The book beside a window this wide is drawn over it.
const NARROW: i32 = 379;

pub(super) fn inventory(kit: &mut Kit) -> (Open, Vec<Target>) {
    let mut targets = vec![Target::Crafted];
    targets.extend((0..4).map(|i| Target::Slot(SlotRef::Ui(ui::CRAFTING_2X2 + i))));
    targets.extend((0..4).map(|i| Target::Slot(SlotRef::Armor(i))));
    targets.extend(player_targets());
    targets.push(Target::Slot(SlotRef::Offhand));
    let mouse = kit.view.mouse.map(|m| m as f32);
    let window = Window::Inventory { figure: player_figure(), last_mouse: mouse, effects: effects(kit) };
    let title = kit.tr("container.crafting");
    recipe_screen(kit, Kind::Crafting, window, title, targets)
}

pub(super) fn crafting(kit: &mut Kit) -> (Open, Vec<Target>) {
    let mut targets = vec![Target::Crafted];
    targets.extend((ui::CRAFTING_3X3..=ui::CRAFTING_3X3_LAST).map(|i| Target::Slot(SlotRef::Ui(i))));
    targets.extend(player_targets());
    let title = kit.title("container.crafting");
    recipe_screen(kit, Kind::Crafting, Window::Crafting, title, targets)
}

pub(super) fn player_figure() -> Figure {
    Figure { entity: "minecraft:player".into(), bb_height: 1.8, scale: Some(1.0), fall_flying: false }
}

/// The player's effects as the server sent them: the time is not counted down, and the colour
/// (`MobEffect.getColor`) is not known (0).
pub(super) fn effects(kit: &Kit) -> Vec<Effect> {
    let me = kit.view.me.map_or(&[][..], |m| &m.effects[..]);
    let java = |id: i32| acacia_ui::effects::EFFECTS.iter().find(|e| e.0 == id).map(|e| e.1);
    me.iter()
        .filter_map(|&(id, ticks, _, ambient)| Some(Effect { id: format!("minecraft:{}", java(id)?), duration: ticks, amplifier: 0, ambient, color: 0 }))
        .collect()
}

pub(super) fn recipe_screen(kit: &mut Kit, kind: Kind, window: Window, title: Text, targets: Vec<Target>) -> (Open, Vec<Target>) {
    let book = book(kit, kind);
    let inventory = kit.inventory_title();
    let mut s = RecipeScreen::new(book, window, title, inventory, kit.env);
    fill(kit, &mut s.base, &targets);
    (Open::Recipe(s), targets)
}

/// Whether `stack`'s name or id holds the search's words (`SearchTree`, roughly).
fn found(kit: &Kit, stack: &Stack, sought: &str) -> bool {
    let id = super::super::items::java_id(stack, kit.items);
    let name = super::super::items::item_stack(stack, kit.items, kit.lang, 0);
    let name = acacia_ui::java::tooltip::item::hover_name(&name, kit.env.lang, kit.env.game).plain().to_lowercase();
    name.contains(sought) || id.contains(sought)
}

fn book(kit: &mut Kit, kind: Kind) -> Book {
    let [width, height] = kit.size();
    let state = &kit.state.book;
    let sought = state.search.text().to_lowercase();
    let craftable: &[Stack] = if kind == Kind::Crafting { &kit.view.inventory.craftable } else { &[] };
    let mut collections = Vec::new();
    let mut listed = Vec::new();
    for (i, stack) in craftable.iter().enumerate() {
        let (_, key) = kit.listed(stack);
        collections.push(Collection { entries: vec![Entry { selected: true, craftable: true, display: vec![key], cells: Vec::new() }] });
        if sought.trim().is_empty() || found(kit, stack, sought.trim()) {
            listed.push(i);
        }
    }
    let (_, compass) = kit.listed(&plain("minecraft:compass"));
    let caret = state.search.before_caret().chars().count();
    let search = Search { value: state.search.text(), cursor: caret, highlight: caret, display: 0, focused: state.focused, blink_ms: kit.ticks() as u64 * 50 };
    let items = kit.keys.keys().map(|&k| (k, super::super::items::item_stack(&kit.keys[&k], kit.items, kit.lang, k))).collect();
    Book {
        kind,
        visible: state.visible,
        narrow: width < NARROW,
        width,
        height,
        filtering: state.filtering,
        time: kit.ticks() as f32,
        tabs: vec![Tab { search: true, icons: vec![compass], collections: listed, anim: 0.0 }],
        selected_tab: 0,
        collections,
        page: state.page,
        button_anims: Vec::new(),
        search,
        overlay: state.overlay,
        ghost: Vec::new(),
        items,
        hovered: None,
    }
}

/// The trader's screen. What Java shows of an offer that Bedrock does not send is neutral: no
/// special price or demand, a sold-out offer used up once of once, the experience to come 0.
pub(super) fn merchant(kit: &mut Kit) -> (Open, Vec<Target>) {
    let Some(trade) = kit.view.inventory.trade.as_ref() else { return inventory(kit) };
    let mut targets = vec![Target::Slot(SlotRef::Ui(ui::TRADE_INGREDIENT_1)), Target::Slot(SlotRef::Ui(ui::TRADE_INGREDIENT_2)), Target::None];
    targets.extend(player_targets());
    let title = Text::literal(kit.bedrock.translate(&trade.title, &[]));
    let mut s = Merchant::new(kit.size()[0], kit.size()[1], title, kit.inventory_title());
    fill(kit, &mut s.base, &targets);
    s.offers = trade
        .offers
        .iter()
        .map(|o| {
            let (base_cost_a, a) = kit.listed(&o.price);
            let cost_b = o.second_price.as_ref().map(|b| kit.listed(b));
            let (result, r) = kit.listed(&o.goods);
            let used = i32::from(!o.open);
            let keys = [a, a, cost_b.as_ref().map_or(-1, |b| b.1), r];
            Offer { base_cost_a, cost_b: cost_b.map(|b| b.0), result, uses: used, max_uses: 1, special_price: 0, demand: 0, price_multiplier: 0.0, keys }
        })
        .collect();
    (s.level, s.xp, s.future_xp) = (trade.tier + 1, trade.xp, 0);
    (s.show_progress_bar, s.can_restock) = (trade.level.is_some(), false);
    (s.shop_item, s.scroll_off) = (kit.state.shop_item, kit.state.scroll_off);
    (Open::Merchant(s), targets)
}
