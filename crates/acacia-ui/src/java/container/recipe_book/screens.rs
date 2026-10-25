//! `AbstractRecipeBookScreen` and its subclasses: `CraftingScreen`, `FurnaceScreen`,
//! `BlastFurnaceScreen`, `SmokerScreen` and `InventoryScreen`, each a container window moved right
//! of its recipe book when that is open, with the book's toggle button.

use super::{Book, Kind};
use crate::java::container::effects::{self, Effect};
use crate::java::container::entity::{Figure, follows_mouse};
use crate::java::container::{Base, Env, LABEL, Screen, Slot, blit, carried, contents, extract_slot, hovered_tooltip, menus, transparent_background};
use crate::java::gui::Gui;
use crate::java::mth::ceil;
use crate::java::text::Text;

/// Which window the book sits beside.
#[derive(Clone, Debug)]
pub enum Window {
    Crafting,
    /// A furnace's `ContainerData`: lit time, lit duration, cooking progress, cooking total.
    Furnace([i32; 4]),
    /// The player's figure, the mouse of the last frame it follows, and the active effects.
    Inventory { figure: Figure, last_mouse: [f32; 2], effects: Vec<Effect> },
}

pub struct RecipeScreen {
    pub base: Base,
    pub book: Book,
    pub window: Window,
}

const IMAGE: [i32; 2] = [176, 166];

impl RecipeScreen {
    /// The screen after `init`: `book` already holds its state, the window placed beside it.
    pub fn new(book: Book, window: Window, title: Text, inventory: Text, env: &Env) -> RecipeScreen {
        let mut slots = Vec::new();
        match &window {
            Window::Crafting => {
                slots.push(Slot::new(0, 124, 35));
                menus::grid(&mut slots, 3, 3, 30, 17);
            }
            Window::Furnace(_) => {
                slots.extend([Slot::new(0, 56, 17), Slot::new(1, 56, 53), Slot::new(2, 116, 35)]);
            }
            Window::Inventory { .. } => {
                slots.push(Slot::new(0, 154, 28));
                menus::grid(&mut slots, 2, 2, 98, 18);
                let armor = ["container/slot/helmet", "container/slot/chestplate", "container/slot/leggings", "container/slot/boots"];
                for (i, icon) in armor.into_iter().enumerate() {
                    slots.push(Slot { icon: Some(icon), max_stack: 1, ..Slot::new(5 + i, 8, 8 + i as i32 * 18) });
                }
            }
        }
        menus::standard_inventory(&mut slots, 8, 84);
        if matches!(window, Window::Inventory { .. }) {
            slots.push(Slot { icon: Some("container/slot/shield"), ..Slot::new(45, 77, 62) });
        }
        let mut base = Base::new(book.width, book.height, IMAGE, title, inventory, slots);
        base.left = book.screen_left(IMAGE[0]);
        match &window {
            Window::Crafting => base.title_label[0] = 29,
            Window::Furnace(_) => base.title_label[0] = (IMAGE[0] - env.font.width(&base.title)) / 2,
            Window::Inventory { .. } => base.title_label[0] = 97,
        }
        RecipeScreen { base, book, window }
    }

    fn narrow_book(&self) -> bool {
        self.book.visible && self.book.narrow
    }

    /// `getRecipeBookButtonPosition`.
    fn button(&self) -> [i32; 4] {
        let (left, height) = (self.base.left, self.base.height);
        match self.window {
            Window::Crafting => [left + 5, height / 2 - 49, 20, 18],
            Window::Furnace(_) => [left + 20, height / 2 - 49, 20, 18],
            Window::Inventory { .. } => [left + 104, height / 2 - 22, 20, 18],
        }
    }

    fn furnace_sprites(&self) -> (&'static str, &'static str, &'static str) {
        match self.book.kind {
            Kind::BlastFurnace => ("gui/container/blast_furnace", "container/blast_furnace/lit_progress", "container/blast_furnace/burn_progress"),
            Kind::Smoker => ("gui/container/smoker", "container/smoker/lit_progress", "container/smoker/burn_progress"),
            _ => ("gui/container/furnace", "container/furnace/lit_progress", "container/furnace/burn_progress"),
        }
    }
}

/// `AbstractFurnaceMenu.getLitProgress` and `getBurnProgress`.
fn furnace_progress(data: [i32; 4]) -> (f32, f32) {
    let duration = if data[1] == 0 { 200 } else { data[1] };
    let lit = (data[0] as f32 / duration as f32).clamp(0.0, 1.0);
    let burn = if data[3] != 0 && data[2] != 0 { (data[2] as f32 / data[3] as f32).clamp(0.0, 1.0) } else { 0.0 };
    (lit, burn)
}

impl Screen for RecipeScreen {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn background(&mut self, gui: &mut Gui, _env: &Env, _mouse: [i32; 2], _partial: f32) {
        transparent_background(gui);
        let (xo, yo) = (self.base.left, self.base.top);
        match &self.window {
            Window::Crafting => blit(gui, "gui/container/crafting_table", xo, yo, [0.0, 0.0], IMAGE[0], IMAGE[1]),
            Window::Furnace(data) => {
                let (texture, lit_sprite, burn_sprite) = self.furnace_sprites();
                blit(gui, texture, xo, yo, [0.0, 0.0], IMAGE[0], IMAGE[1]);
                let (lit, burn) = furnace_progress(*data);
                if data[0] > 0 {
                    let h = ceil(lit * 13.0) + 1;
                    gui.region(lit_sprite, [14, 14], [0, 14 - h], xo + 56, yo + 36 + 14 - h, 14, h);
                }
                gui.region(burn_sprite, [24, 16], [0, 0], xo + 79, yo + 34, ceil(burn * 24.0), 16);
            }
            Window::Inventory { figure, last_mouse, .. } => {
                blit(gui, "gui/container/inventory", xo, yo, [0.0, 0.0], IMAGE[0], IMAGE[1]);
                follows_mouse(gui, [xo + 26, yo + 8, xo + 75, yo + 78], 30, 0.0625, *last_mouse, figure);
            }
        }
    }

    fn widgets(&mut self, gui: &mut Gui, _env: &Env, mouse: [i32; 2], _partial: f32) {
        let rect = self.button();
        let sprite = if super::widgets::hovered(rect, mouse) { "recipe_book/button_highlighted" } else { "recipe_book/button" };
        gui.sprite(sprite, rect[0], rect[1], rect[2], rect[3]);
    }

    fn labels(&mut self, gui: &mut Gui, _env: &Env, _mouse: [i32; 2]) {
        let b = &self.base;
        gui.text(b.title.clone(), b.title_label[0], b.title_label[1], LABEL, false);
        if !matches!(self.window, Window::Inventory { .. }) {
            gui.text(b.inventory_title.clone(), b.inventory_label[0], b.inventory_label[1], LABEL, false);
        }
    }

    fn slots(&mut self, gui: &mut Gui, env: &Env, _mouse: [i32; 2]) {
        for i in 0..self.base.slots.len() {
            if self.base.slots[i].active {
                extract_slot(&mut self.base, gui, env, i);
            }
        }
        let big = !matches!(self.window, Window::Inventory { .. });
        self.book.ghost_render(gui, &self.base, big);
    }

    fn hovering(&self, x: i32, y: i32, w: i32, h: i32, mouse: [i32; 2]) -> bool {
        let b = &self.base;
        let (mx, my) = (mouse[0] - b.left, mouse[1] - b.top);
        !self.narrow_book() && mx >= x - 1 && mx < x + w + 1 && my >= y - 1 && my < y + h + 1
    }

    fn render(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2], partial: f32) {
        if let Window::Inventory { effects, .. } = &self.window {
            effects::extract(&mut self.base, gui, env.font, env.lang, effects, mouse, 20.0);
        }
        if self.narrow_book() {
            self.background(gui, env, mouse, partial);
        } else {
            contents(self, gui, env, mouse, partial);
        }
        gui.next_stratum();
        self.book.render(gui, env, mouse, partial, &mut self.base.tooltip);
        gui.next_stratum();
        carried(&self.base, gui, mouse);
        hovered_tooltip(self, env, mouse);
        self.book.tooltip(env, mouse, self.base.hovered, &mut self.base.tooltip);
    }
}
