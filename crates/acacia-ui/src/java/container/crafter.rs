//! `CrafterScreen`: disabled grid slots, the redstone indicator and the toggle tooltip over an
//! empty enabled slot.

use super::{Base, Env, Screen, Slot, blit, contents, carried, extract_slot, hovered_tooltip, menus, transparent_background};
use crate::java::gui::{Gui, Metrics};
use crate::java::text::Text;
use crate::java::tooltip::{Line, Positioner, Tooltip};

pub struct Crafter {
    pub base: Base,
    /// `ContainerData`: slots 0..9 disabled (1) or not, then powered.
    pub data: [i32; 10],
}

/// `CrafterMenu`'s slots: the 3×3 grid, the inventory, the result.
pub fn slots() -> Vec<Slot> {
    let mut slots = Vec::new();
    menus::grid(&mut slots, 3, 3, 26, 17);
    menus::standard_inventory(&mut slots, 8, 84);
    slots.push(Slot { highlightable: false, fake: true, ..Slot::new(45, 134, 35) });
    slots
}

impl Crafter {
    pub fn new(width: i32, height: i32, title: Text, inventory: Text, data: [i32; 10], font: &dyn Metrics) -> Crafter {
        let mut base = Base::new(width, height, [176, 166], title, inventory, slots());
        base.title_label[0] = (base.image_width - font.width(&base.title)) / 2;
        Crafter { base, data }
    }

    fn disabled(&self, index: usize) -> bool {
        index < 9 && self.data[index] == 1
    }
}

impl Screen for Crafter {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn background(&mut self, gui: &mut Gui, _env: &Env, _mouse: [i32; 2], _partial: f32) {
        transparent_background(gui);
        let b = &self.base;
        let (x, y) = ((b.width - b.image_width) / 2, (b.height - b.image_height) / 2);
        blit(gui, "gui/container/crafter", x, y, [0.0, 0.0], b.image_width, b.image_height);
    }

    fn slot(&mut self, gui: &mut Gui, env: &Env, i: usize, _mouse: [i32; 2]) {
        if self.disabled(self.base.slots[i].index) {
            let s = &self.base.slots[i];
            gui.sprite("container/crafter/disabled_slot", s.x - 1, s.y - 1, 18, 18);
        } else {
            extract_slot(&mut self.base, gui, env, i);
        }
    }

    fn render(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2], partial: f32) {
        contents(self, gui, env, mouse, partial);
        carried(&self.base, gui, mouse);
        hovered_tooltip(self, env, mouse);
        let b = &self.base;
        let redstone = if self.data[9] == 1 { "container/crafter/powered_redstone" } else { "container/crafter/unpowered_redstone" };
        gui.sprite(redstone, b.width / 2 + 9, b.height / 2 - 48, 16, 16);
        let toggle = b.hovered.map(|i| &b.slots[i]).is_some_and(|s| s.index < 9 && !self.disabled(s.index) && !s.has_item());
        if toggle && b.carried().is_none() {
            let lines = vec![Line::Text(Text::translate("gui.togglable_slot", &[], env.lang))];
            let t = Tooltip { lines, x: mouse[0], y: mouse[1], positioner: Positioner::Default, style: None, extra_space: false };
            self.base.tooltip.set(t, false);
        }
    }
}
