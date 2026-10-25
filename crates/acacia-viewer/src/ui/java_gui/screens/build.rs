//! Building a screen from the viewer's state: which screen the open window is, each slot's stack
//! and target, the carried stack and drag, and the draw keys the painter looks stacks up by.

use std::collections::HashMap;
use std::time::Instant;

use acacia_bot::items::SlotRef;
use acacia_bot::proto::types::{GameMode, WindowType};
use acacia_render::item::JavaItems;
use acacia_ui::inventory::Bench;
use acacia_ui::java::container::item_combiner::Station;
use acacia_ui::java::container::{Base, CARRIED, Env, QUICK, quick_craft_count};
use acacia_ui::java::text::Text;
use acacia_ui::java::tooltip::stack::ItemStack;

use super::super::items::item_stack;
use super::super::lang::JavaLang;
use super::{Built, JavaGui, JavaScreen, Open, State, Target, benches, containers, creative, player};
use crate::control::{Inventory, Stack};
use crate::creative_items::CreativeEntry;
use crate::me::Me;

/// What a screen is built from.
pub struct View<'a> {
    pub inventory: &'a Inventory,
    /// The creative inventory shows, on this tab (`acacia_ui::inventory::TABS`).
    pub creative: Option<usize>,
    pub entries: &'a [CreativeEntry],
    /// The anvil's name box or the creative search: its text and caret (in chars).
    pub name: (String, usize),
    pub me: Option<&'a Me>,
    /// The GUI's size (rounded up) and the mouse, in GUI pixels.
    pub size: [i32; 2],
    pub mouse: [f64; 2],
    pub now: Instant,
}

impl<'a> View<'a> {
    pub fn new(inventory: &'a Inventory, java: &JavaScreen<'a>, me: Option<&'a Me>, size: [i32; 2], mouse: [f64; 2], now: Instant) -> View<'a> {
        let name = (java.name.text(), java.name.before_caret().chars().count());
        View { inventory, creative: java.creative, entries: java.entries, name, me, size, mouse, now }
    }
}

/// Draw keys besides the slots' (their menu indices) and the fixed ones: lists' stacks.
const LIST_KEYS: i32 = 10_000;

pub(super) struct Kit<'a> {
    pub env: &'a Env<'a>,
    pub items: Option<&'a JavaItems>,
    pub lang: &'a JavaLang,
    /// Bedrock's translations, for what the server names in them (a trader's title).
    pub bedrock: &'a acacia_ui::lang::Lang,
    pub view: &'a View<'a>,
    pub state: &'a State,
    pub keys: HashMap<i32, Stack>,
    next: i32,
}

impl Kit<'_> {
    pub fn tr(&self, key: &str) -> Text {
        Text::translate(key, &[], self.env.lang)
    }

    /// `stack` drawn under `key`.
    pub fn keyed(&mut self, stack: &Stack, key: i32) -> ItemStack {
        self.keys.insert(key, stack.clone());
        item_stack(stack, self.items, self.lang, key)
    }

    /// `stack` under a key of its own.
    pub fn listed(&mut self, stack: &Stack) -> (ItemStack, i32) {
        let key = self.next;
        self.next += 1;
        (self.keyed(stack, key), key)
    }

    pub fn size(&self) -> [i32; 2] {
        self.view.size
    }

    /// The player's levels and whether they build with infinite materials.
    pub fn station(&self, data: Vec<i32>) -> Station {
        let me = self.view.me;
        Station { xp: me.map_or(0, |m| m.xp_level), creative: me.is_some_and(|m| m.game_mode == GameMode::Creative), data }
    }

    /// The window's title: the name it was given, else `key`.
    pub fn title(&self, key: &str) -> Text {
        self.view.inventory.custom_title.as_deref().map_or_else(|| self.tr(key), Text::legacy)
    }

    pub fn inventory_title(&self) -> Text {
        self.tr("container.inventory")
    }

    pub fn ticks(&self) -> i32 {
        self.state.ticks(self.view.now)
    }
}

/// `addStandardInventorySlots`' targets: the three rows, then the hotbar.
pub(super) fn player_targets() -> Vec<Target> {
    (9..36).chain(0..9).map(|i| Target::Slot(SlotRef::Main(i))).collect()
}

/// The stack a target shows.
pub(super) fn stack_at<'a>(view: &'a View, state: &State, target: &Target) -> Option<&'a Stack> {
    let inv = view.inventory;
    match *target {
        Target::Slot(SlotRef::Main(i)) => inv.main.get(usize::from(i))?.as_ref(),
        Target::Slot(SlotRef::Armor(i)) => inv.armor.get(usize::from(i))?.as_ref(),
        Target::Slot(SlotRef::Offhand) => inv.offhand.as_ref(),
        Target::Slot(SlotRef::Cursor) => inv.cursor.as_ref(),
        Target::Slot(SlotRef::Container(i)) => inv.container.as_ref()?.slots.get(usize::from(i))?.as_ref(),
        Target::Slot(SlotRef::Ui(i)) => inv.ui.get(usize::from(i))?.as_ref(),
        Target::Crafted => inv.crafted.as_ref(),
        Target::Cut(_) => state.pick.and_then(|i| inv.picks.get(i)).map(|p| &p.1),
        Target::Loom(_) => state.pick.and_then(|i| inv.patterns.get(i)).and_then(|p| p.1.as_ref()),
        Target::Creative(id) => view.entries.iter().find(|e| e.id == id).map(|e| &e.stack),
        Target::Destroy | Target::None => None,
    }
}

/// `base`'s slots from their targets, the carried stack and the quick-craft drag in progress.
pub(super) fn fill(kit: &mut Kit, base: &mut Base, targets: &[Target]) {
    let view = kit.view;
    for (slot, target) in base.slots.iter_mut().zip(targets) {
        slot.item = stack_at(view, kit.state, target).map(|s| kit.keyed(s, slot.index as i32));
        if !matches!(target, Target::Slot(_)) {
            slot.drag_target = false;
        }
    }
    let Some(cursor) = &view.inventory.cursor else { return };
    base.carried = Some(kit.keyed(cursor, CARRIED));
    base.distinct_keys = true;
    base.quick = kit.state.pointer.quick();
    base.recalculate_quick_craft();
    let (Some(quick), Some(carried)) = (base.quick.clone(), base.carried().cloned()) else { return };
    for &i in &quick.slots {
        let slot = &base.slots[i];
        let carry = slot.item.as_ref().map_or(0, |s| s.count);
        let n = (quick_craft_count(quick.slots.len(), quick.kind, &carried) + carry).min(carried.max_stack_size().min(slot.max_stack));
        kit.keys.insert(QUICK + i as i32, Stack { count: n.max(0) as u16, ..cursor.clone() });
    }
    kit.keys.insert(QUICK - 1, Stack { count: quick.remainder.max(0) as u16, ..cursor.clone() });
}

impl JavaGui {
    /// The open screen for `view` as `state` leaves it.
    pub(in crate::ui) fn build(&self, items: Option<&JavaItems>, bedrock: &acacia_ui::lang::Lang, view: &View, state: &State) -> Built {
        self.with_env(|env| {
            let mut kit = Kit { env, items, lang: &self.lang, bedrock, view, state, keys: HashMap::new(), next: LIST_KEYS };
            let (open, targets) = choose(&mut kit);
            Built { open, keys: kit.keys, targets }
        })
    }
}

/// Which screen the open window is.
fn choose(kit: &mut Kit) -> (Open, Vec<Target>) {
    let inv = kit.view.inventory;
    if inv.trade.is_some() {
        return player::merchant(kit);
    }
    if kit.view.creative.is_some() && inv.container.is_none() && inv.bench.is_none() {
        return creative::creative(kit);
    }
    if let Some(station) = inv.station.filter(|_| inv.container.is_some()) {
        return containers::station(kit, station);
    }
    match inv.bench {
        Some(Bench::Crafting) => return player::crafting(kit),
        Some(bench) => return benches::bench(kit, bench),
        None => {}
    }
    match &inv.container {
        Some(_) if inv.window == Some(WindowType::Crafter) => containers::crafter(kit),
        Some(rows) => containers::chest(kit, rows.slots.len()),
        None => player::inventory(kit),
    }
}
