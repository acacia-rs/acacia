//! `AbstractMountInventoryScreen`: `HorseInventoryScreen` (horses, donkeys, mules, llamas, camels)
//! and `NautilusInventoryScreen`, the mount's equipment and chest slots beside its figure.

use super::entity::{Figure, follows_mouse};
use super::{Base, Env, Screen, Slot, blit, menus, transparent_background};
use crate::java::gui::Gui;
use crate::java::text::Text;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Horse,
    Llama,
    Nautilus,
}

pub struct Mount {
    pub base: Base,
    pub kind: Kind,
    pub columns: i32,
    pub figure: Figure,
    /// The mouse of the last frame, which the figure follows (`xMouse`, `yMouse`).
    pub last_mouse: [f32; 2],
}

impl Mount {
    /// `saddle` and `armor`: whether the mount can wear them (the menu slots' `isActive`).
    #[allow(clippy::too_many_arguments)]
    pub fn new(kind: Kind, width: i32, height: i32, title: Text, inventory: Text, columns: i32, saddle: bool, armor: bool, figure: Figure) -> Mount {
        let armor_icon = match kind {
            Kind::Horse => "container/slot/horse_armor",
            Kind::Llama => "container/slot/llama_armor",
            Kind::Nautilus => "container/slot/nautilus_armor_inventory",
        };
        let equipment = |index, y, icon, active| Slot { icon: Some(icon), max_stack: 1, active, ..Slot::new(index, 8, y) };
        let mut slots = vec![equipment(0, 18, "container/slot/saddle", saddle), equipment(1, 36, armor_icon, armor)];
        if kind != Kind::Nautilus && columns > 0 {
            menus::grid(&mut slots, columns, 3, 80, 18);
        }
        menus::standard_inventory(&mut slots, 8, 84);
        let base = Base::new(width, height, [176, 166], title, inventory, slots);
        Mount { base, kind, columns, figure, last_mouse: [0.0, 0.0] }
    }
}

impl Screen for Mount {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn background(&mut self, gui: &mut Gui, _env: &Env, _mouse: [i32; 2], _partial: f32) {
        transparent_background(gui);
        let b = &self.base;
        let (xo, yo) = ((b.width - b.image_width) / 2, (b.height - b.image_height) / 2);
        let texture = if self.kind == Kind::Nautilus { "gui/container/nautilus" } else { "gui/container/horse" };
        blit(gui, texture, xo, yo, [0.0, 0.0], b.image_width, b.image_height);
        if self.columns > 0 && self.kind != Kind::Nautilus {
            gui.region("container/horse/chest_slots", [90, 54], [0, 0], xo + 79, yo + 17, self.columns * 18, 54);
        }
        for (slot, y) in [(0, yo + 17), (1, yo + 35)] {
            if b.slots[slot].active {
                gui.sprite("container/slot", xo + 7, y, 18, 18);
            }
        }
        follows_mouse(gui, [xo + 26, yo + 18, xo + 78, yo + 70], 17, 0.25, self.last_mouse, &self.figure);
    }
}
