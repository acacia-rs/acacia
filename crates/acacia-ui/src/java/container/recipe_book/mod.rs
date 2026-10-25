//! The recipe book beside the crafting, furnace and inventory windows (`RecipeBookComponent`): the
//! background, the search box, the category tabs, the craftable filter, a page of recipe buttons
//! with its arrows and number, a button's alternatives overlay, the ghost recipe in the menu's
//! slots and the hovered button's tooltip. Which recipes the book knows, which the grid fits and
//! which the inventory can craft is game state ([`Collection`]); this is what it draws from it.
//! Ported from Horizon's `recipe_book_screen.rs` (MIT).

pub mod page;
pub mod screens;
pub mod widgets;

use std::collections::HashMap;

use super::{Base, Env};
use crate::java::gui::{Blend, Gui, GuiDraw};
use crate::java::mth::{WHITE, floor};
use crate::java::tooltip::item;
use crate::java::tooltip::stack::ItemStack;
use crate::java::tooltip::{Deferred, Line, Positioner, Tooltip};

pub const IMAGE_WIDTH: i32 = 147;
pub const IMAGE_HEIGHT: i32 = 166;

/// The menu's `RecipeBookType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Crafting,
    Furnace,
    BlastFurnace,
    Smoker,
}

impl Kind {
    pub fn furnace(self) -> bool {
        self != Kind::Crafting
    }

    /// `getRecipeFilterName`.
    fn filter_name(self) -> &'static str {
        match self {
            Kind::Crafting => "gui.recipebook.toggleRecipes.craftable",
            Kind::Furnace => "gui.recipebook.toggleRecipes.smeltable",
            Kind::BlastFurnace => "gui.recipebook.toggleRecipes.blastable",
            Kind::Smoker => "gui.recipebook.toggleRecipes.smokable",
        }
    }
}

/// An overlay button's ingredient: its offset in the button (`3 + gridX * 7`) and the stacks it cycles.
#[derive(Clone, Debug, Default)]
pub struct Cell {
    pub x: i32,
    pub y: i32,
    pub items: Vec<i32>,
}

/// A `RecipeDisplayEntry` of a collection, the stacks by draw key.
#[derive(Clone, Debug, Default)]
pub struct Entry {
    /// The menu can show it (`RecipeCollection.selected`).
    pub selected: bool,
    /// The inventory can craft it (`RecipeCollection.craftable`, only ever selected ones).
    pub craftable: bool,
    /// `resultItems`.
    pub display: Vec<i32>,
    /// Its overlay button's ingredients.
    pub cells: Vec<Cell>,
}

/// A `RecipeCollection`: recipes grouped under one button.
#[derive(Clone, Debug, Default)]
pub struct Collection {
    pub entries: Vec<Entry>,
}

impl Collection {
    pub fn has_any_selected(&self) -> bool {
        self.entries.iter().any(|e| e.selected)
    }

    pub fn has_craftable(&self) -> bool {
        self.entries.iter().any(|e| e.craftable)
    }

    /// `getSelectedRecipes(filtering ? CRAFTABLE : ANY)`.
    pub fn fitting(&self, filtering: bool) -> Vec<&Entry> {
        self.entries.iter().filter(|e| if filtering { e.craftable } else { e.selected }).collect()
    }
}

/// A `RecipeBookTabButton`.
#[derive(Clone, Debug, Default)]
pub struct Tab {
    /// A `SearchRecipeBookCategory` tab: shown whatever it holds.
    pub search: bool,
    /// Primary and optional secondary icon.
    pub icons: Vec<i32>,
    /// Indices into [`Book::collections`].
    pub collections: Vec<usize>,
    /// Squeeze animation time left.
    pub anim: f32,
}

/// The search `EditBox`'s state, in characters.
#[derive(Clone, Debug, Default)]
pub struct Search {
    pub value: String,
    pub cursor: usize,
    pub highlight: usize,
    pub display: usize,
    pub focused: bool,
    /// Milliseconds since focused.
    pub blink_ms: u64,
}

/// A ghost recipe slot: the menu slot (index into `Base::slots`), whether it is the result, its stacks.
#[derive(Clone, Debug, Default)]
pub struct Ghost {
    pub slot: usize,
    pub result: bool,
    pub items: Vec<i32>,
}

/// `RecipeBookComponent` and its page.
#[derive(Clone, Debug)]
pub struct Book {
    pub kind: Kind,
    pub visible: bool,
    pub narrow: bool,
    pub width: i32,
    pub height: i32,
    pub filtering: bool,
    /// `time`, in ticks; recipes cycle every 30.
    pub time: f32,
    pub tabs: Vec<Tab>,
    pub selected_tab: usize,
    pub collections: Vec<Collection>,
    /// The page asked for, clamped to the pages there are.
    pub page: usize,
    /// The page's buttons' squeeze animation time left.
    pub button_anims: Vec<f32>,
    pub search: Search,
    /// The page button whose alternatives are open.
    pub overlay: Option<usize>,
    pub ghost: Vec<Ghost>,
    /// The stacks drawn, by key.
    pub items: HashMap<i32, ItemStack>,
    /// The page button under the mouse in the last render.
    pub hovered: Option<usize>,
}

impl Book {
    fn x_offset(&self) -> i32 {
        if self.narrow { 0 } else { 86 }
    }

    /// `getXOrigin`, `getYOrigin`.
    pub fn origin(&self) -> [i32; 2] {
        [(self.width - IMAGE_WIDTH) / 2 - self.x_offset(), (self.height - IMAGE_HEIGHT) / 2]
    }

    /// `updateScreenPosition`: the window's left edge.
    pub fn screen_left(&self, image_width: i32) -> i32 {
        if self.visible && !self.narrow { 177 + (self.width - image_width - 200) / 2 } else { (self.width - image_width) / 2 }
    }

    /// `slotSelectTime.currentIndex()`.
    pub fn index(&self) -> usize {
        floor(self.time / 30.0).max(0) as usize
    }

    fn tab_visible(&self, t: &Tab) -> bool {
        t.search || t.collections.iter().any(|&c| self.collections[c].has_any_selected())
    }

    /// `updateCollections`: the selected tab's collections the page lists.
    pub fn listed(&self) -> Vec<usize> {
        let Some(tab) = self.tabs.get(self.selected_tab) else { return Vec::new() };
        let shown = |&&c: &&usize| self.collections[c].has_any_selected() && (!self.filtering || self.collections[c].has_craftable());
        tab.collections.iter().filter(shown).copied().collect()
    }

    /// `extractRenderState`; widget tooltips go to `tooltip`.
    pub fn render(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2], partial: f32, tooltip: &mut Deferred) {
        if !self.visible {
            return;
        }
        self.time += partial;
        let [xo, yo] = self.origin();
        gui.draws.push(GuiDraw::Blit {
            texture: "gui/recipe_book".into(),
            blend: Blend::Textured,
            x: xo,
            y: yo,
            w: IMAGE_WIDTH,
            h: IMAGE_HEIGHT,
            uv: [1.0, 1.0],
            src: [IMAGE_WIDTH, IMAGE_HEIGHT],
            tex: [256, 256],
            color: WHITE,
        });
        widgets::search_box(gui, env, &self.search, xo + 25, yo + 13, mouse);
        self.tabs_render(gui, partial);
        let filter = [xo + 110, yo + 12, 26, 16];
        let hovered = widgets::hovered(filter, mouse);
        let prefix = if self.kind.furnace() { "recipe_book/furnace_filter_" } else { "recipe_book/filter_" };
        let state = if self.filtering { "enabled" } else { "disabled" };
        gui.sprite(format!("{prefix}{state}{}", if hovered { "_highlighted" } else { "" }), filter[0], filter[1], filter[2], filter[3]);
        if hovered {
            let key = if self.filtering { self.kind.filter_name() } else { "gui.recipebook.toggleRecipes.all" };
            widgets::tooltip(tooltip, env, key, filter, mouse);
        }
        page::render(self, gui, env, mouse, partial, tooltip);
    }

    fn tabs_render(&mut self, gui: &mut Gui, partial: f32) {
        let x = (self.width - IMAGE_WIDTH) / 2 - self.x_offset() - 30;
        let mut y = (self.height - IMAGE_HEIGHT) / 2 + 3;
        for i in 0..self.tabs.len() {
            if !self.tab_visible(&self.tabs[i]) {
                continue;
            }
            let selected = i == self.selected_tab;
            let tab = &mut self.tabs[i];
            widgets::tab(gui, tab, selected, x, y, partial);
            y += 27;
        }
    }

    /// `extractTooltip`: the hovered button's stack, else a hovered ghost slot's.
    pub fn tooltip(&self, env: &Env, mouse: [i32; 2], hovered_slot: Option<usize>, tooltip: &mut Deferred) {
        if !self.visible {
            return;
        }
        if let Some(b) = self.hovered.filter(|_| self.overlay.is_none()) {
            let (key, many) = page::display(self, b);
            if let Some(stack) = key.and_then(|k| self.items.get(&k)) {
                let mut lines = item_lines(stack, env);
                if many {
                    lines.push(Line::Text(crate::java::text::Text::translate("gui.recipebook.moreRecipes", &[], env.lang)));
                }
                tooltip.set(stack_tooltip(stack, lines, mouse), false);
            }
        }
        let index = self.index();
        let ghost = hovered_slot.and_then(|s| self.ghost.iter().find(|g| g.slot == s && !g.items.is_empty()));
        if let Some(stack) = ghost.and_then(|g| self.items.get(&g.items[index % g.items.len()])) {
            tooltip.set(stack_tooltip(stack, item_lines(stack, env), mouse), false);
        }
    }

    /// `GhostSlots.extractRenderState` under the window's origin; drawn before [`Book::render`] steps the time.
    pub fn ghost_render(&self, gui: &mut Gui, base: &Base, big_result: bool) {
        let index = self.index();
        for g in &self.ghost {
            let slot = &base.slots[g.slot];
            let (x, y) = (slot.x, slot.y);
            if g.result && big_result {
                gui.fill(x - 4, y - 4, x + 20, y + 20, 0x30FF_0000);
            } else {
                gui.fill(x, y, x + 16, y + 16, 0x30FF_0000);
            }
            let key = g.items[index % g.items.len()];
            gui.item(key, x, y, 0);
            gui.fill(x, y, x + 16, y + 16, 0x30FF_FFFF);
            if g.result {
                gui.decorations(key, x, y, None);
            }
        }
    }
}

fn item_lines(stack: &ItemStack, env: &Env) -> Vec<Line> {
    item::tooltip_lines(stack, &env.details(stack, false)).into_iter().map(Line::Text).collect()
}

/// `setComponentTooltipForNextFrame` with the stack's tooltip style.
fn stack_tooltip(stack: &ItemStack, lines: Vec<Line>, mouse: [i32; 2]) -> Tooltip {
    let style = stack.get("tooltip_style").and_then(|v| v.as_str()).map(str::to_owned);
    Tooltip { lines, x: mouse[0], y: mouse[1], positioner: Positioner::Default, style, extra_space: false }
}
