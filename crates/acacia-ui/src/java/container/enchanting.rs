//! `EnchantmentScreen`: the window, the book (a picture-in-picture between its last two ticks'
//! open and flip), the three options with their galactic names and level costs (disabled,
//! enabled or hovered), and the hovered option's clue tooltip with its lapis and level costs.

use super::enchanting_names::EnchantmentNames;
use super::item_combiner::{self, Station};
use super::{Base, Env, Screen, blit, carried, contents, hovered_tooltip, transparent_background};
use crate::java::gui::{Gui, GuiDraw};
use crate::java::mth::lerp;
use crate::java::picture::Picture;
use crate::java::text::{Lang, Text};
use crate::java::tooltip::{Line, Positioner, Tooltip};
use crate::java::widget::measure::split;

const GRAY: u32 = 0xAAAAAA;
const RED: u32 = 0xFF5555;

/// An enchantment as the clue names it.
#[derive(Clone, Debug)]
pub struct Clue {
    pub description: Text,
    pub curse: bool,
    pub max_level: i32,
}

impl Clue {
    /// `Enchantment.getFullname(enchantment, level)`.
    pub fn full_name(&self, level: i32, lang: Lang) -> Text {
        let mut name = self.description.clone();
        if level != 1 || self.max_level != 1 {
            name = name.append(" ").append(Text::translate(&format!("enchantment.level.{level}"), &[], lang));
        }
        name.with_color(if self.curse { RED } else { GRAY })
    }
}

pub struct Enchanting {
    pub base: Base,
    /// Data: three costs, the seed, three clue ids, three clue levels.
    pub station: Station,
    pub clues: [Option<Clue>; 3],
    /// `[open, oOpen]` and `[flip, oFlip]`.
    pub open: [f32; 2],
    pub flip: [f32; 2],
}

impl Enchanting {
    #[allow(clippy::too_many_arguments)]
    pub fn new(width: i32, height: i32, title: Text, inventory: Text, station: Station, clues: [Option<Clue>; 3], open: [f32; 2], flip: [f32; 2]) -> Enchanting {
        let mut slots = item_combiner::slots(&[(15, 47), (35, 47)], (8, 84));
        slots[1].icon = Some("container/slot/lapis_lazuli");
        Enchanting { base: Base::new(width, height, [176, 166], title, inventory, slots), station, clues, open, flip }
    }

    fn gold(&self) -> i32 {
        self.base.slots[1].item.as_ref().filter(|s| !s.is_empty()).map_or(0, |s| s.count)
    }

    /// `mouseClicked`'s own part: the option `clickMenuButton` takes (lapis, levels and an item
    /// to enchant enough for it).
    pub fn mouse_clicked(&self, mouse: [f64; 2]) -> Option<usize> {
        let b = &self.base;
        let (xo, yo) = ((b.width - b.image_width) / 2, (b.height - b.image_height) / 2);
        let s = &self.station;
        (0..3).find(|&i| {
            let (xx, yy) = (mouse[0] - f64::from(xo + 60), mouse[1] - f64::from(yo + 14 + 19 * i as i32));
            let cost = i as i32 + 1;
            let paid = s.creative || self.gold() >= cost;
            let able = s.data(i) > 0 && b.slots[0].has_item() && (s.creative || s.xp >= cost && s.xp >= s.data(i));
            xx >= 0.0 && yy >= 0.0 && xx < 108.0 && yy < 19.0 && paid && able
        })
    }

    fn options(&self, gui: &mut Gui, env: &Env, mouse: [i32; 2], xo: i32, yo: i32) {
        let mut names = EnchantmentNames::new(i64::from(self.station.data(3)));
        let gold = self.gold();
        for i in 0..3 {
            let (left, y) = (xo + 60, yo + 14 + 19 * i as i32);
            let text_x = left + 20;
            let cost = self.station.data(i);
            if cost == 0 {
                gui.sprite("container/enchanting_table/enchantment_slot_disabled", left, y, 108, 19);
                continue;
            }
            let cost_text = Text::literal(cost.to_string());
            let text_width = 86 - env.font.width(&cost_text);
            let message = names.random_name(env.font, text_width);
            let level = i + 1;
            let color = if (gold < level as i32 || self.station.xp < cost) && !self.station.creative {
                gui.sprite("container/enchanting_table/enchantment_slot_disabled", left, y, 108, 19);
                gui.sprite(format!("container/enchanting_table/level_{level}_disabled"), left + 1, y + 1, 16, 16);
                wrapped(gui, env, &message, text_x, y + 2, text_width, 0xFF00_0000 | ((0x685E4A & 0xFE_FEFE) >> 1));
                0xFF40_7F10
            } else {
                let (xx, yy) = (mouse[0] - left, mouse[1] - y);
                let mut col = 0xFF68_5E4A;
                if xx >= 0 && yy >= 0 && xx < 108 && yy < 19 {
                    gui.sprite("container/enchanting_table/enchantment_slot_highlighted", left, y, 108, 19);
                    col = 0xFFFF_FF80;
                } else {
                    gui.sprite("container/enchanting_table/enchantment_slot", left, y, 108, 19);
                }
                gui.sprite(format!("container/enchanting_table/level_{level}"), left + 1, y + 1, 16, 16);
                wrapped(gui, env, &message, text_x, y + 2, text_width, col);
                0xFF80_FF20
            };
            let w = env.font.width(&cost_text);
            gui.text(cost_text, text_x + 86 - w, y + 2 + 7, color, true);
        }
    }

    fn clue_tooltip(&mut self, env: &Env, mouse: [i32; 2]) {
        let (gold, s) = (self.gold(), &self.station);
        for i in 0..3 {
            let Some(clue) = &self.clues[i] else { continue };
            let (min_level, level, cost) = (s.data(i), s.data(7 + i), i as i32 + 1);
            if !(self.hovering(60, 14 + 19 * i as i32, 108, 17, mouse) && min_level > 0 && level >= 0) {
                continue;
            }
            let tr = |key: &str, args: &[Text], color: u32| Text::translate(key, args, env.lang).with_color(color);
            let mut lines = vec![tr("container.enchant.clue", &[clue.full_name(level, env.lang)], 0xFFFFFF)];
            if !s.creative {
                lines.push(Text::default());
                if s.xp < min_level {
                    lines.push(tr("container.enchant.level.requirement", &[Text::literal(min_level.to_string())], RED));
                } else {
                    let n = [Text::literal(cost.to_string())];
                    let color = if gold >= cost { GRAY } else { RED };
                    lines.push(if cost == 1 { tr("container.enchant.lapis.one", &[], color) } else { tr("container.enchant.lapis.many", &n, color) });
                    lines.push(if cost == 1 { tr("container.enchant.level.one", &[], GRAY) } else { tr("container.enchant.level.many", &n, GRAY) });
                }
            }
            let lines = lines.into_iter().map(Line::Text).collect();
            let t = Tooltip { lines, x: mouse[0], y: mouse[1], positioner: Positioner::Default, style: None, extra_space: false };
            self.base.tooltip.set(t, false);
            break;
        }
    }
}

/// `textWithWordWrap(font, text, x, y, width, color, false)`.
fn wrapped(gui: &mut Gui, env: &Env, text: &Text, x: i32, y: i32, width: i32, color: u32) {
    for (n, line) in split(env.font, text, width).into_iter().enumerate() {
        gui.text(line, x, y + 9 * n as i32, color, false);
    }
}

impl Screen for Enchanting {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn background(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2], partial: f32) {
        transparent_background(gui);
        let b = &self.base;
        let (xo, yo) = ((b.width - b.image_width) / 2, (b.height - b.image_height) / 2);
        blit(gui, "gui/container/enchanting_table", xo, yo, [0.0, 0.0], b.image_width, b.image_height);
        let (open, flip) = (lerp(partial, self.open[1], self.open[0]), lerp(partial, self.flip[1], self.flip[0]));
        let (x0, y0) = (xo + 14, yo + 14);
        let texture = "textures/entity/enchantment/enchanting_table_book.png".to_owned();
        gui.draws.push(GuiDraw::Picture(Picture::Book { texture, scale: 40.0, open, flip, rect: [x0, y0, x0 + 38, y0 + 31] }));
        self.options(gui, env, mouse, xo, yo);
    }

    fn render(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2], partial: f32) {
        contents(self, gui, env, mouse, partial);
        carried(&self.base, gui, mouse);
        hovered_tooltip(self, env, mouse);
        self.clue_tooltip(env, mouse);
    }
}
