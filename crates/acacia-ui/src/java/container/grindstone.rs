//! `GrindstoneScreen`: the window and its error icon when inputs give no result.

use super::item_combiner::{self, error_icon};
use super::{Base, Env, Screen};
use crate::java::gui::Gui;
use crate::java::text::Text;

pub struct Grindstone {
    pub base: Base,
}

impl Grindstone {
    pub fn new(width: i32, height: i32, title: Text, inventory: Text) -> Grindstone {
        let slots = item_combiner::slots(&[(49, 19), (49, 40), (129, 34)], (8, 84));
        Grindstone { base: Base::new(width, height, [176, 166], title, inventory, slots) }
    }
}

impl Screen for Grindstone {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn background(&mut self, gui: &mut Gui, _env: &Env, _mouse: [i32; 2], _partial: f32) {
        item_combiner::background(gui, &self.base, "gui/container/grindstone");
        error_icon(gui, &self.base, "container/grindstone/error", 92, 31, 2);
    }
}
