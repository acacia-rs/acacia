//! `CreativeModeInventoryScreen`: the selected tab's window with the other tabs' buttons around
//! it, the item picker's 9 × 5 grid scrolled over the tab's items (or the player's inventory with
//! its figure and the destroy slot), the search box, the scroll bar and the tabs' tooltips.
//! Which items a tab lists, and the search box's results, are the game's data.

use super::creative_search::SearchBox;
use super::effects::{self, Effect};
use super::entity::{Figure, follows_mouse};
use super::{Base, Env, LABEL, Screen, Slot, carried, contents, hovered_tooltip, transparent_background};
use crate::java::gui::{Blend, Gui, GuiDraw};
use crate::java::mth::WHITE;
use crate::java::text::Text;
use crate::java::tooltip::item::{self, Context};
use crate::java::tooltip::stack::ItemStack;
use crate::java::tooltip::{Line, Positioner, Tooltip};

/// The draw key of tab `i`'s icon is `ICONS + i`.
pub const ICONS: i32 = 1000;
pub const GRID: usize = 45;
const IMAGE: [i32; 2] = [195, 136];
const BLUE: u32 = 0x5555FF;
const DARK_PURPLE: u32 = 0xAA00AA;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabType {
    Category,
    Hotbar,
    Search,
    Inventory,
}

/// A `CreativeModeTab` as the screen reads it.
#[derive(Clone, Debug)]
pub struct Tab {
    pub kind: TabType,
    pub top: bool,
    pub column: i32,
    pub right: bool,
    pub show_title: bool,
    pub can_scroll: bool,
    /// `getBackgroundTexture`'s path (`textures/gui/container/creative_inventory/tab_items.png`).
    pub background: String,
    pub title: Text,
}

#[derive(Clone, Debug)]
pub struct Creative {
    pub base: Base,
    /// `CreativeModeTabs.tabs()`.
    pub tabs: Vec<Tab>,
    pub selected: usize,
    /// `menu.items.size()`.
    pub items: usize,
    pub scroll: f32,
    /// Shown on the search tab only.
    pub search: SearchBox,
    pub figure: Figure,
    pub effects: Vec<Effect>,
    /// The tabs (indices into `tabs`) whose search contents hold the hovered stack.
    pub member: Vec<usize>,
    /// `visibleTags` in its set's order, and the hovered stack's tags.
    pub visible_tags: Vec<String>,
    pub item_tags: Vec<String>,
}

/// `InventoryMenu`'s slot `i` as `(inventory slot, empty icon)`; the crafting slots have none.
fn inventory_slot(i: usize) -> (Option<usize>, Option<&'static str>) {
    match i {
        5..=8 => (Some(39 - (i - 5)), Some(["container/slot/helmet", "container/slot/chestplate", "container/slot/leggings", "container/slot/boots"][i - 5])),
        9..=35 => (Some(i), None),
        36..=44 => (Some(i - 36), None),
        45 => (Some(40), Some("container/slot/shield")),
        _ => (None, None),
    }
}

/// `selectTab(INVENTORY)`'s slot positions.
fn inventory_position(i: usize) -> (i32, i32) {
    match i {
        5..=8 => (54 + (i as i32 - 5) / 2 * 54, 6 + (i as i32 - 5) % 2 * 27),
        0..=4 => (-2000, -2000),
        45 => (35, 20),
        _ => {
            let pos = i as i32 - 9;
            (9 + pos % 9 * 18, if i >= 36 { 112 } else { 54 + pos / 9 * 18 })
        }
    }
}

impl Creative {
    /// The screen on `tabs[selected]`, its slots empty.
    pub fn new(width: i32, height: i32, tabs: Vec<Tab>, selected: usize, figure: Figure) -> Creative {
        let inventory = tabs[selected].kind == TabType::Inventory;
        let slots = if inventory {
            let mut slots: Vec<Slot> = (0..46).map(|i| {
                let (x, y) = inventory_position(i);
                Slot { icon: inventory_slot(i).1, ..Slot::new(i, x, y) }
            }).collect();
            slots.push(Slot::new(46, 173, 112));
            slots
        } else {
            let grid = (0..GRID).map(|i| Slot::new(i, 9 + i as i32 % 9 * 18, 18 + i as i32 / 9 * 18));
            grid.chain((0..9).map(|i| Slot::new(GRID + i, 9 + i as i32 * 18, 112))).collect()
        };
        let base = Base::new(width, height, IMAGE, Text::default(), Text::default(), slots);
        let search = SearchBox::new(base.left + 82, base.top + 6);
        Creative { base, tabs, selected, items: 0, scroll: 0.0, search, figure, effects: Vec::new(), member: Vec::new(), visible_tags: Vec::new(), item_tags: Vec::new() }
    }

    pub fn tab(&self) -> &Tab {
        &self.tabs[self.selected]
    }

    /// The player's inventory slot `slot` holds `stack`.
    pub fn set_inventory(&mut self, slot: usize, stack: ItemStack) {
        let inventory = self.tab().kind == TabType::Inventory;
        for (i, s) in self.base.slots.iter_mut().enumerate() {
            let held = if inventory { inventory_slot(i).0 } else { (i >= GRID).then(|| i - GRID) };
            if held == Some(slot) {
                s.item = Some(stack.clone());
            }
        }
    }

    fn row_count(&self) -> i32 {
        (self.items as i32 + 8) / 9 - 5
    }

    pub fn can_scroll(&self) -> bool {
        self.items > GRID
    }

    /// `menu.items` (`present(i)` for each entry that is not empty) scrolled to `scroll`.
    pub fn set_items(&mut self, items: usize, present: impl Fn(usize) -> Option<ItemStack>, scroll: f32) {
        self.items = items;
        self.scroll = if self.can_scroll() { scroll } else { 0.0 };
        if self.tab().kind == TabType::Inventory {
            return;
        }
        let row = (((self.scroll * self.row_count() as f32) as f64 + 0.5) as i32).max(0) as usize;
        for i in 0..GRID {
            let n = i + row * 9;
            self.base.slots[i].item = if n < items { present(n) } else { None };
        }
    }

    fn tab_x(&self, tab: &Tab) -> i32 {
        if tab.right { self.base.image_width - 27 * (7 - tab.column) + 1 } else { 27 * tab.column }
    }

    fn tab_y(&self, tab: &Tab) -> i32 {
        if tab.top { -32 } else { self.base.image_height }
    }

    fn tab_button(&self, gui: &mut Gui, i: usize) {
        let tab = &self.tabs[i];
        let x = self.base.left + self.tab_x(tab);
        let y = if tab.top { self.base.top - 28 } else { self.base.top + self.base.image_height - 4 };
        let row = if tab.top { "top" } else { "bottom" };
        let state = if i == self.selected { "selected" } else { "unselected" };
        gui.sprite(format!("container/creative_inventory/tab_{row}_{state}_{}", tab.column.clamp(0, 6) + 1), x, y, 26, 32);
        gui.item(ICONS + i as i32, x + 5, y + 8 + if tab.top { 1 } else { -1 }, 0);
    }

    fn is_creative_slot(&self) -> bool {
        self.tab().kind != TabType::Inventory && self.base.hovered.is_some_and(|i| i < GRID)
    }

    fn tooltip(&mut self, text: Text, mouse: [i32; 2]) {
        let t = Tooltip { lines: vec![Line::Text(text)], x: mouse[0], y: mouse[1], positioner: Positioner::Default, style: None, extra_space: false };
        self.base.tooltip.set(t, false);
    }
}

impl Screen for Creative {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn background(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2], _partial: f32) {
        transparent_background(gui);
        for i in (0..self.tabs.len()).filter(|&i| i != self.selected) {
            self.tab_button(gui, i);
        }
        let b = &self.base;
        let texture = self.tab().background.trim_start_matches("textures/").trim_end_matches(".png").to_owned();
        let (x, y, w, h) = (b.left, b.top, b.image_width, b.image_height);
        gui.draws.push(GuiDraw::Blit { texture: texture.into(), blend: Blend::Textured, x, y, w, h, uv: [0.0, 0.0], src: [w, h], tex: [256, 256], color: WHITE });
        if self.tab().kind == TabType::Search {
            self.search.extract(gui, env.font);
        }
        if self.tab().can_scroll {
            let sprite = if self.can_scroll() { "container/creative_inventory/scroller" } else { "container/creative_inventory/scroller_disabled" };
            gui.sprite(sprite, x + 175, y + 18 + ((112 - 17) as f32 * self.scroll) as i32, 12, 15);
        }
        self.tab_button(gui, self.selected);
        if self.tab().kind == TabType::Inventory {
            follows_mouse(gui, [x + 73, y + 6, x + 105, y + 49], 20, 0.0625, [mouse[0] as f32, mouse[1] as f32], &self.figure);
        }
    }

    fn labels(&mut self, gui: &mut Gui, _env: &Env, _mouse: [i32; 2]) {
        if self.tab().show_title {
            gui.text(self.tab().title.clone(), 8, 6, LABEL, false);
        }
    }

    fn tooltip_lines(&self, stack: &ItemStack, env: &Env) -> Vec<Text> {
        let creative_slot = self.is_creative_slot();
        let mut lines = item::lines(stack, &Context { lang: env.lang, advanced: env.advanced, creative: creative_slot });
        let kind = self.tab().kind;
        if lines.is_empty() || (kind == TabType::Category && creative_slot) {
            return lines;
        }
        if kind == TabType::Search && creative_slot {
            for tag in self.visible_tags.iter().filter(|t| self.item_tags.contains(t)) {
                lines.insert(1, Text::literal(format!("#{tag}")).with_color(DARK_PURPLE));
            }
        }
        let mut at = 1;
        for (i, tab) in self.tabs.iter().enumerate() {
            if tab.kind != TabType::Search && self.member.contains(&i) {
                lines.insert(at, tab.title.clone().with_color(BLUE));
                at += 1;
            }
        }
        lines
    }

    fn render(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2], partial: f32) {
        effects::extract(&mut self.base, gui, env.font, env.lang, &self.effects, mouse, 20.0);
        contents(self, gui, env, mouse, partial);
        carried(&self.base, gui, mouse);
        hovered_tooltip(self, env, mouse);
        let hovered = self.tabs.iter().find(|tab| self.hovering(self.tab_x(tab) + 3, self.tab_y(tab) + 3, 21, 27, mouse));
        if let Some(title) = hovered.map(|tab| tab.title.clone()) {
            self.tooltip(title, mouse);
        }
        if self.tab().kind == TabType::Inventory && self.hovering(173, 112, 16, 16, mouse) {
            self.tooltip(Text::translate("inventory.binSlot", &[], env.lang), mouse);
        }
    }
}
