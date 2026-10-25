//! The screens that are a window texture over their menu: `ContainerScreen` (chests, 1 to 6 rows),
//! `ShulkerBoxScreen`, `DispenserScreen` (dispenser, dropper) and `HopperScreen`.

use super::{Base, Env, Screen, blit, menus, transparent_background};
use crate::java::gui::{Gui, Metrics};
use crate::java::text::Text;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Chest { rows: i32 },
    Shulker,
    Dispenser,
    Hopper,
}

pub struct Simple {
    pub base: Base,
    pub kind: Kind,
}

impl Simple {
    /// The screen opened `width` × `height` big; `inventory` is the player inventory's name.
    pub fn new(kind: Kind, width: i32, height: i32, title: Text, inventory: Text, font: &dyn Metrics) -> Simple {
        let (image, slots) = match kind {
            Kind::Chest { rows } => ([176, 114 + rows * 18], menus::chest(rows)),
            Kind::Shulker => ([176, 167], menus::shulker()),
            Kind::Dispenser => ([176, 166], menus::dispenser()),
            Kind::Hopper => ([176, 133], menus::hopper()),
        };
        let mut base = Base::new(width, height, image, title, inventory, slots);
        if kind == Kind::Dispenser {
            base.title_label[0] = (base.image_width - font.width(&base.title)) / 2;
        }
        Simple { base, kind }
    }
}

impl Screen for Simple {
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
        let texture = match self.kind {
            Kind::Chest { rows } => {
                blit(gui, "gui/container/generic_54", x, y, [0.0, 0.0], b.image_width, rows * 18 + 17);
                blit(gui, "gui/container/generic_54", x, y + rows * 18 + 17, [0.0, 126.0], b.image_width, 96);
                return;
            }
            Kind::Shulker => "gui/container/shulker_box",
            Kind::Dispenser => "gui/container/dispenser",
            Kind::Hopper => "gui/container/hopper",
        };
        blit(gui, texture, x, y, [0.0, 0.0], b.image_width, b.image_height);
    }
}
