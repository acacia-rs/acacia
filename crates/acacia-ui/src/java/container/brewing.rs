//! `BrewingStandScreen`: fuel bar, brewing arrow and bubbles over the window, from the menu's data.

use super::{Base, Env, Screen, Slot, blit, menus, transparent_background};
use crate::java::gui::{Gui, Metrics};
use crate::java::text::Text;

const BUBBLES: [i32; 7] = [29, 24, 20, 16, 11, 6, 0];

pub struct Brewing {
    pub base: Base,
    /// `ContainerData`: brewing ticks, fuel, total brewing ticks, total fuel.
    pub data: [i32; 4],
}

/// `BrewingStandMenu`'s slots.
pub fn slots() -> Vec<Slot> {
    let potion = |i, x, y| Slot { max_stack: 1, icon: Some("container/slot/potion"), ..Slot::new(i, x, y) };
    let mut slots = vec![potion(0, 56, 51), potion(1, 79, 58), potion(2, 102, 51), Slot::new(3, 79, 17)];
    slots.push(Slot { icon: Some("container/slot/brewing_fuel"), ..Slot::new(4, 17, 17) });
    menus::standard_inventory(&mut slots, 8, 84);
    slots
}

/// `Mth.positiveCeilDiv`.
fn ceil_div(a: i32, b: i32) -> i32 {
    -(-a).div_euclid(b)
}

impl Brewing {
    pub fn new(width: i32, height: i32, title: Text, inventory: Text, data: [i32; 4], font: &dyn Metrics) -> Brewing {
        let mut base = Base::new(width, height, [176, 166], title, inventory, slots());
        base.title_label[0] = (base.image_width - font.width(&base.title)) / 2;
        Brewing { base, data }
    }
}

impl Screen for Brewing {
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
        blit(gui, "gui/container/brewing_stand", x, y, [0.0, 0.0], b.image_width, b.image_height);
        let [ticks, fuel, total_ticks, total_fuel] = self.data;
        let fuel_length = if total_fuel > 0 { ceil_div(18 * fuel, total_fuel).clamp(0, 18) } else { 0 };
        if fuel_length > 0 {
            gui.region("container/brewing_stand/fuel_length", [18, 4], [0, 0], x + 60, y + 44, fuel_length, 4);
        }
        if ticks > 0 && total_ticks > 0 {
            let length = (28.0 * (1.0 - ticks as f32 / total_ticks as f32)) as i32;
            if length > 0 {
                gui.region("container/brewing_stand/brew_progress", [9, 28], [0, 0], x + 97, y + 16, 9, length);
            }
            let length = BUBBLES[(ticks / 2 % 7) as usize];
            if length > 0 {
                gui.region("container/brewing_stand/bubbles", [12, 29], [0, 29 - length], x + 63, y + 14 + 29 - length, 12, length);
            }
        }
    }
}
