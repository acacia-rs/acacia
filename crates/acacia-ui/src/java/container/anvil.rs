//! `AnvilScreen`: the window, the name field (focused, its sprite greyed with no input), the error
//! icon, and the cost line ("Too Expensive!" from 40 levels, red when the result can't be taken).

use super::item_combiner::{self, Station, error_icon, has};
use super::{Base, Env, LABEL, Screen};
use crate::java::gui::{Gui, Metrics};
use crate::java::text::{Lang, Text};
use crate::java::tooltip::game::Game;
use crate::java::tooltip::item::hover_name;
use crate::java::widget::edit_box::EditBox;
use crate::java::widget::{Ctx, render};

const COST: u32 = 0xFF80_FF20;
const RED: u32 = 0xFFFF_6060;

pub struct Anvil {
    pub base: Base,
    pub station: Station,
    pub name: EditBox,
}

impl Anvil {
    /// The screen once its slots are filled: the name field holds the input's name (as
    /// `slotChanged` sets it), then `typed` if given; `cursor_shown` is the blink's phase.
    pub fn new(base: Base, station: Station, typed: Option<&str>, cursor_shown: bool, font: &dyn Metrics, lang: Lang, game: &Game) -> Anvil {
        let mut base = base;
        base.title_label[0] = 60;
        let (x, y) = ((base.width - base.image_width) / 2, (base.height - base.image_height) / 2);
        let mut name = EditBox::new(x + 62, y + 24, 103, 12, Text::translate("container.repair", &[], lang));
        name.text_color = 0xFFFF_FFFF;
        name.text_color_uneditable = 0xFFFF_FFFF;
        name.invert_highlight = false;
        name.bordered = false;
        name.max_length = 50;
        name.set_value(font, "");
        name.update_text_position(font);
        let input = base.slots[0].item.clone().filter(|s| !s.is_empty());
        name.set_value(font, &input.as_ref().map_or(String::new(), |s| hover_name(s, lang, game).plain()));
        name.editable = input.is_some();
        if let (Some(typed), Some(_)) = (typed, &input) {
            name.set_value(font, typed);
        }
        name.base.focused = true;
        name.focused_time = if cursor_shown { -150 } else { -450 };
        Anvil { base, station, name }
    }

    /// `ItemCombinerScreen.slots`: two inputs, the result, the inventory.
    pub fn slots() -> Vec<super::Slot> {
        item_combiner::slots(&[(27, 47), (76, 47), (134, 47)], (8, 84))
    }
}

impl Screen for Anvil {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn background(&mut self, gui: &mut Gui, _env: &Env, _mouse: [i32; 2], _partial: f32) {
        let b = &self.base;
        item_combiner::background(gui, b, "gui/container/anvil");
        error_icon(gui, b, "container/anvil/error", 99, 45, 2);
        let field = if has(b, 0) { "container/anvil/text_field" } else { "container/anvil/text_field_disabled" };
        gui.sprite(field, b.left + 59, b.top + 20, 110, 16);
    }

    fn widgets(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2], _partial: f32) {
        let mut ctx = Ctx::new(gui, env.font, env.lang, (mouse[0], mouse[1]));
        render(&mut self.name, &mut ctx);
    }

    fn labels(&mut self, gui: &mut Gui, env: &Env, _mouse: [i32; 2]) {
        let b = &self.base;
        gui.text(b.title.clone(), b.title_label[0], b.title_label[1], LABEL, false);
        gui.text(b.inventory_title.clone(), b.inventory_label[0], b.inventory_label[1], LABEL, false);
        let cost = self.station.data(0);
        if cost <= 0 {
            return;
        }
        let s = &self.station;
        let (line, color) = if cost >= 40 && !s.creative {
            (Text::translate("container.repair.expensive", &[], env.lang), RED)
        } else if !has(b, 2) {
            return;
        } else {
            let takeable = (s.creative || s.xp >= cost) && cost > 0;
            (Text::translate("container.repair.cost", &[Text::literal(cost.to_string())], env.lang), if takeable { COST } else { RED })
        };
        let x = b.image_width - 8 - env.font.width(&line) - 2;
        gui.fill(x - 2, 67, b.image_width - 8, 79, 0x4F00_0000);
        gui.text(line, x, 69, color, true);
    }
}
