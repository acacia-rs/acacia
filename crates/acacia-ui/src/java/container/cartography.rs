//! `CartographyTableScreen`: the window, the error icon, and the preview of the resulting map:
//! scaled, duplicated (two overlapping copies, the second a stratum above), locked, or plain.

use super::item_combiner::{self, has};
use super::{Base, Env, Screen, blit, transparent_background};
use crate::java::gui::{Gui, GuiDraw, PoseOp};
use crate::java::picture::Picture;
use crate::java::text::Text;

/// A map's saved data as the client holds it.
#[derive(Clone, Copy, Debug)]
pub struct MapData {
    pub scale: i32,
    pub locked: bool,
    pub decorations: usize,
}

pub struct Cartography {
    pub base: Base,
    /// The data of the map in the first slot, if the client has it.
    pub map: Option<MapData>,
    /// The first slot's stack is in `#extendable_maps`.
    pub extendable: bool,
}

impl Cartography {
    pub fn new(width: i32, height: i32, title: Text, inventory: Text, map: Option<MapData>, extendable: bool) -> Cartography {
        let slots = item_combiner::slots(&[(15, 15), (15, 52), (145, 39)], (8, 84));
        let mut base = Base::new(width, height, [176, 166], title, inventory, slots);
        base.title_label[1] -= 2;
        Cartography { base, map, extendable }
    }

    fn map(&self, gui: &mut Gui, x: i32, y: i32, scale: f32) {
        if let Some(data) = self.map {
            gui.push(vec![PoseOp::Translate(x as f32, y as f32), PoseOp::Scale(scale, scale)]);
            let key = self.base.slots[0].item.as_ref().and_then(|s| s.key);
            gui.draws.push(GuiDraw::Picture(Picture::Map { decorations: data.decorations, key }));
            gui.pop();
        }
    }
}

impl Screen for Cartography {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn background(&mut self, gui: &mut Gui, _env: &Env, _mouse: [i32; 2], _partial: f32) {
        transparent_background(gui);
        let b = &self.base;
        let (x, y) = (b.left, b.top);
        blit(gui, "gui/container/cartography_table", x, y, [0.0, 0.0], b.image_width, b.image_height);
        let additional = b.slots[1].item.as_ref().filter(|_| has(b, 1)).map(|s| s.id.as_str());
        let (duplicate, scale, lock) = (additional == Some("minecraft:map"), additional == Some("minecraft:paper"), additional == Some("minecraft:glass_pane"));
        let mut locked = false;
        if let Some(data) = self.map {
            if data.locked {
                locked = true;
                if scale || lock {
                    gui.sprite("container/cartography_table/error", x + 35, y + 31, 28, 21);
                }
            }
            if scale && (!self.extendable || data.scale >= 4) {
                locked = true;
                gui.sprite("container/cartography_table/error", x + 35, y + 31, 28, 21);
            }
        }
        if scale && !locked {
            gui.sprite("container/cartography_table/scaled_map", x + 67, y + 13, 66, 66);
            self.map(gui, x + 85, y + 31, 0.226);
        } else if duplicate {
            gui.sprite("container/cartography_table/duplicated_map", x + 67 + 16, y + 13, 50, 66);
            self.map(gui, x + 86, y + 16, 0.34);
            gui.next_stratum();
            gui.sprite("container/cartography_table/duplicated_map", x + 67, y + 13 + 16, 50, 66);
            self.map(gui, x + 70, y + 32, 0.34);
        } else if lock {
            gui.sprite("container/cartography_table/map", x + 67, y + 13, 66, 66);
            self.map(gui, x + 71, y + 17, 0.45);
            gui.sprite("container/cartography_table/locked", x + 118, y + 60, 10, 14);
        } else {
            gui.sprite("container/cartography_table/map", x + 67, y + 13, 66, 66);
            self.map(gui, x + 71, y + 17, 0.45);
        }
    }
}
