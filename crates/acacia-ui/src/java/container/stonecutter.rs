//! `StonecutterScreen`: the recipes the input can be cut into, a 4 × 3 grid of buttons with
//! their results over a scroller.

use super::input::{Press, drag_scroll};
use super::{Base, Env, Screen, Slot, blit, carried, contents, hovered_tooltip, item_tooltip, menus, transparent_background};
use crate::java::gui::Gui;
use crate::java::mth::clamp;
use crate::java::text::Text;
use crate::java::tooltip::stack::ItemStack;

pub struct Stonecutter {
    pub base: Base,
    /// The visible recipes' icons (`optionDisplay().resolveForFirstStack`) and their draw keys.
    pub recipes: Vec<(ItemStack, i32)>,
    /// `menu.getSelectedRecipeIndex()`.
    pub selected: i32,
    scroll_offs: f32,
    start_index: usize,
    display_recipes: bool,
}

impl Stonecutter {
    pub fn new(width: i32, height: i32, title: Text, inventory: Text) -> Stonecutter {
        let mut slots = vec![Slot::new(0, 20, 33), Slot::new(1, 143, 33)];
        menus::standard_inventory(&mut slots, 8, 84);
        let mut base = Base::new(width, height, [176, 166], title, inventory, slots);
        base.title_label[1] -= 1;
        Stonecutter { base, recipes: Vec::new(), selected: -1, scroll_offs: 0.0, start_index: 0, display_recipes: false }
    }

    /// `containerChanged`, after the input or its recipes change.
    pub fn container_changed(&mut self) {
        self.display_recipes = self.base.slots[0].has_item() && !self.recipes.is_empty();
        self.scroll_offs = 0.0;
        self.start_index = 0;
    }

    fn scroll_bar_active(&self) -> bool {
        self.display_recipes && self.recipes.len() > 12
    }

    fn offscreen_rows(&self) -> i32 {
        (self.recipes.len() as i32 + 3) / 4 - 3
    }

    /// `mouseScrolled` by `dy` away from any slot.
    pub fn scroll(&mut self, dy: f64) {
        if self.scroll_bar_active() {
            let rows = self.offscreen_rows();
            self.scroll_offs = clamp(self.scroll_offs - dy as f32 / rows as f32, 0.0, 1.0);
            self.start_index = (f64::from(self.scroll_offs * rows as f32) + 0.5) as usize * 4;
        }
    }

    /// `scrollOffs`, to carry to the next frame's screen.
    pub fn scroll_offs(&self) -> f32 {
        self.scroll_offs
    }

    /// `scrollOffs` as an earlier frame left it, with `startIndex` from it.
    pub fn set_scroll(&mut self, offs: f32) {
        if self.scroll_bar_active() {
            self.scroll_offs = offs;
            self.start_index = (f64::from(offs * self.offscreen_rows() as f32) + 0.5) as usize * 4;
        }
    }

    /// `mouseClicked`'s own part: a recipe button `clickMenuButton` takes (not the selected one),
    /// or the scroller.
    pub fn mouse_clicked(&self, mouse: [f64; 2]) -> Option<Press> {
        if !self.display_recipes {
            return None;
        }
        let (xo, yo) = (self.base.left + 52, self.base.top + 14);
        for index in self.start_index..self.start_index + 12 {
            let pos = (index - self.start_index) as i32;
            let (xx, yy) = (mouse[0] - f64::from(xo + pos % 4 * 16), mouse[1] - f64::from(yo + pos / 4 * 18));
            if xx >= 0.0 && yy >= 0.0 && xx < 16.0 && yy < 18.0 && self.selected != index as i32 {
                // Taken past the last recipe too, selecting nothing.
                return Some(Press::Button(index));
            }
        }
        let (x, y) = (self.base.left + 119, self.base.top + 9);
        let on = mouse[0] >= f64::from(x) && mouse[0] < f64::from(x + 12) && mouse[1] >= f64::from(y) && mouse[1] < f64::from(y + 54);
        on.then_some(Press::Scroller)
    }

    /// `mouseDragged` while the scroller is held.
    pub fn mouse_dragged(&mut self, mouse: [f64; 2]) {
        if self.scroll_bar_active() {
            self.scroll_offs = drag_scroll(mouse[1], self.base.top + 14, 54, 7.5);
            self.start_index = (f64::from(self.scroll_offs * self.offscreen_rows() as f32) + 0.5) as usize * 4;
        }
    }

    /// The recipe buttons on screen: `(index, x, y)`, `y` the icon's.
    fn visible(&self) -> Vec<(usize, i32, i32)> {
        let (left, top, start) = (self.base.left + 52, self.base.top + 14, self.start_index);
        (start..(start + 12).min(self.recipes.len()))
            .map(|i| {
                let pos = (i - start) as i32;
                (i, left + pos % 4 * 16, top + pos / 4 * 18 + 2)
            })
            .collect()
    }
}

impl Screen for Stonecutter {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn background(&mut self, gui: &mut Gui, _env: &Env, mouse: [i32; 2], _partial: f32) {
        transparent_background(gui);
        let (xo, yo) = (self.base.left, self.base.top);
        blit(gui, "gui/container/stonecutter", xo, yo, [0.0, 0.0], self.base.image_width, self.base.image_height);
        let sy = (41.0 * self.scroll_offs) as i32;
        let sprite = if self.scroll_bar_active() { "container/stonecutter/scroller" } else { "container/stonecutter/scroller_disabled" };
        gui.sprite(sprite, xo + 119, yo + 15 + sy, 12, 15);
        for (i, x, y) in self.visible() {
            let sprite = if i as i32 == self.selected {
                "container/stonecutter/recipe_selected"
            } else if mouse[0] >= x && mouse[1] >= y && mouse[0] < x + 16 && mouse[1] < y + 18 {
                "container/stonecutter/recipe_highlighted"
            } else {
                "container/stonecutter/recipe"
            };
            gui.sprite(sprite, x, y - 1, 16, 18);
        }
        for (i, x, y) in self.visible() {
            gui.item(self.recipes[i].1, x, y, 0);
        }
    }

    fn render(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2], partial: f32) {
        contents(self, gui, env, mouse, partial);
        carried(self.base(), gui, mouse);
        hovered_tooltip(self, env, mouse);
        if !self.display_recipes {
            return;
        }
        let hit = self.visible().into_iter().find(|&(_, x, y)| mouse[0] >= x && mouse[0] < x + 16 && mouse[1] >= y && mouse[1] < y + 18);
        if let Some((i, _, _)) = hit {
            let t = item_tooltip(self, &self.recipes[i].0, env, mouse);
            self.base.tooltip.set(t, false);
        }
    }
}
