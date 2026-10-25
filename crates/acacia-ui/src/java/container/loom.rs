//! `LoomScreen`: the patterns the banner can take, a 4 × 4 grid of buttons each showing its
//! pattern on a grey flag, a scroller, and the result banner (a picture) or the too-many-patterns
//! error.

use super::{Base, Env, Screen, Slot, blit, menus, text_tooltip, transparent_background};
use crate::java::gui::{Gui, GuiDraw, PoseOp};
use crate::java::mth::clamp;
use crate::java::picture::Picture;
use crate::java::text::Text;
use crate::java::tooltip::stack::ItemStack;

/// `DyeColor.GRAY.getTextureDiffuseColor()`.
const GRAY: u32 = 0xFF47_4F52;
const BANNER_ATLAS: &str = "minecraft:textures/atlas/banner_patterns.png";

/// A selectable `BannerPattern`: its `translationKey` and `Sheets.getBannerSprite` texture.
#[derive(Clone, Debug, Default)]
pub struct Pattern {
    pub translation_key: String,
    pub sprite: String,
}

/// A banner stack's `banner_patterns` layers as `"<pattern id> <dye>"`.
fn layers(stack: &ItemStack) -> Vec<String> {
    let layers = stack.get("banner_patterns").and_then(|v| v.as_array());
    let layer = |l: &serde_json::Value| format!("{} {}", l["pattern"].as_str().unwrap_or("?"), l["color"].as_str().unwrap_or("white"));
    layers.into_iter().flatten().map(layer).collect()
}

/// `BannerItem.getColor()` from the item id (`minecraft:red_banner`).
fn banner_color(stack: &ItemStack) -> String {
    let id = stack.id.split_once(':').map_or(stack.id.as_str(), |(_, p)| p);
    id.strip_suffix("_banner").unwrap_or(id).to_owned()
}

pub struct Loom {
    pub base: Base,
    /// `menu.getSelectablePatterns()`.
    pub patterns: Vec<Pattern>,
    /// `menu.getSelectedBannerPatternIndex()`.
    pub selected: i32,
    result_patterns: Option<Vec<String>>,
    has_max_patterns: bool,
    display_patterns: bool,
    scroll_offs: f32,
    start_row: i32,
}

impl Loom {
    pub fn new(width: i32, height: i32, title: Text, inventory: Text) -> Loom {
        let mut slots = vec![Slot::new(0, 13, 26), Slot::new(1, 33, 26), Slot::new(2, 23, 45), Slot::new(3, 143, 57)];
        menus::standard_inventory(&mut slots, 8, 84);
        let mut base = Base::new(width, height, [176, 166], title, inventory, slots);
        base.title_label[1] -= 2;
        Loom { base, patterns: Vec::new(), selected: -1, result_patterns: None, has_max_patterns: false, display_patterns: false, scroll_offs: 0.0, start_row: 0 }
    }

    fn total_rows(&self) -> i32 {
        (self.patterns.len() as i32 + 3) / 4
    }

    /// `containerChanged` once the input slots have been set.
    pub fn container_changed(&mut self) {
        let item = |i: usize| self.base.slots[i].item.clone().filter(|s| !s.is_empty());
        let banner = item(0);
        self.has_max_patterns = banner.as_ref().is_some_and(|b| layers(b).len() >= 6);
        self.result_patterns = item(3).filter(|_| !self.has_max_patterns).map(|r| layers(&r));
        self.display_patterns = banner.is_some() && item(1).is_some() && !self.has_max_patterns && !self.patterns.is_empty();
        if self.start_row >= self.total_rows() {
            self.start_row = 0;
            self.scroll_offs = 0.0;
        }
    }

    fn scroll_bar_active(&self) -> bool {
        self.display_patterns && self.patterns.len() > 16
    }

    /// `mouseScrolled` by `dy` away from any slot.
    pub fn scroll(&mut self, dy: f64) {
        let rows = self.total_rows() - 4;
        if self.display_patterns && rows > 0 {
            self.scroll_offs = clamp(self.scroll_offs - dy as f32 / rows as f32, 0.0, 1.0);
            self.start_row = ((self.scroll_offs * rows as f32 + 0.5) as i32).max(0);
        }
    }

    fn dye(&self) -> String {
        let dye = self.base.slots[1].item.as_ref().and_then(|s| s.get("dye")).and_then(|v| v.as_str());
        dye.unwrap_or("white").to_owned()
    }

    fn patterns(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2]) {
        let (x, y) = (self.base.left + 60, self.base.top + 13);
        for row in 0..4 {
            for column in 0..4 {
                let index = ((row + self.start_row) * 4 + column) as usize;
                let Some(pattern) = self.patterns.get(index) else { return };
                let (px, py) = (x + column * 14, y + row * 14);
                let highlighted = mouse[0] >= px && mouse[1] >= py && mouse[0] < px + 14 && mouse[1] < py + 14;
                let sprite = if index as i32 == self.selected {
                    "container/loom/pattern_selected"
                } else if highlighted {
                    let name = Text::translate(&format!("{}.{}", pattern.translation_key, self.dye()), &[], env.lang);
                    self.base.tooltip.set(text_tooltip(name, mouse), false);
                    "container/loom/pattern_highlighted"
                } else {
                    "container/loom/pattern"
                };
                gui.sprite(sprite, px, py, 14, 14);
                banner_on_button(gui, px, py, &self.patterns[index].sprite);
            }
        }
    }
}

/// `extractBannerOnButton`: a grey 5 × 10 flag with the pattern's flag face over it.
fn banner_on_button(gui: &mut Gui, x: i32, y: i32, sprite: &str) {
    gui.push(vec![PoseOp::Translate((x + 4) as f32, (y + 2) as f32)]);
    gui.fill(0, 0, 5, 10, GRAY);
    // The flag face: 21 × 40 of the 64 × 64 pattern texture, one pixel down.
    let (u0, u1, v0, v1) = (0.0f32, 1.0f32, 0.0f32, 1.0f32);
    let pu0 = u0;
    let pu1 = pu0 + (u1 - u0) * 21.0 / 64.0;
    let span = v1 - v0;
    let pv0 = v0 + span / 64.0;
    let pv1 = pv0 + span * 40.0 / 64.0;
    let blit = Picture::SpriteBlit { atlas: BANNER_ATLAS.to_owned(), sprite: sprite.to_owned(), rect: [0, 0, 5, 10], uv: [pu0, pu1, pv0, pv1] };
    gui.draws.push(GuiDraw::Picture(blit));
    gui.pop();
}

impl Screen for Loom {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn background(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2], _partial: f32) {
        transparent_background(gui);
        let (xo, yo) = (self.base.left, self.base.top);
        blit(gui, "gui/container/loom", xo, yo, [0.0, 0.0], self.base.image_width, self.base.image_height);
        for (i, icon) in [(0, "container/slot/banner"), (1, "container/slot/dye"), (2, "container/slot/banner_pattern")] {
            let slot = &self.base.slots[i];
            if !slot.has_item() {
                gui.sprite(icon, xo + slot.x, yo + slot.y, 16, 16);
            }
        }
        let sy = (41.0 * self.scroll_offs) as i32;
        let sprite = if self.scroll_bar_active() { "container/loom/scroller" } else { "container/loom/scroller_disabled" };
        gui.sprite(sprite, xo + 119, yo + 13 + sy, 12, 15);
        let result = &self.base.slots[3];
        match &self.result_patterns {
            Some(patterns) if !self.has_max_patterns => {
                let base = result.item.as_ref().map(banner_color).unwrap_or_default();
                let (x0, y0) = (xo + 141, yo + 8);
                gui.draws.push(GuiDraw::Picture(Picture::Banner { base, patterns: patterns.clone(), rect: [x0, y0, x0 + 20, y0 + 40] }));
            }
            _ if self.has_max_patterns => gui.sprite("container/loom/error", xo + result.x - 5, yo + result.y - 5, 26, 26),
            _ => {}
        }
        if self.display_patterns {
            self.patterns(gui, env, mouse);
        }
    }
}
