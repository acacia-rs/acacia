//! `MerchantScreen`: a trader's offers as a list of seven `TradeOfferButton`s with a scroller,
//! the trade slots, the trader's level in the title and its experience bar.

use super::input::Press;
use super::{Base, Env, LABEL, Screen, Slot, carried, contents, hovered_tooltip, item_tooltip, menus, text_tooltip, transparent_background};
use crate::java::gui::{Blend, Gui, GuiDraw};
use crate::java::mth::{WHITE, floor};
use crate::java::text::Text;
use crate::java::tooltip::stack::ItemStack;

const BUTTONS: usize = 7;
/// `VillagerData.NEXT_LEVEL_XP_THRESHOLDS`.
const XP_THRESHOLDS: [i32; 5] = [0, 10, 70, 150, 250];

/// A `MerchantOffer` and the draw keys of its stacks.
#[derive(Clone, Debug, Default)]
pub struct Offer {
    pub base_cost_a: ItemStack,
    pub cost_b: Option<ItemStack>,
    pub result: ItemStack,
    pub uses: i32,
    pub max_uses: i32,
    pub special_price: i32,
    pub demand: i32,
    pub price_multiplier: f32,
    /// Keys of `baseCostA`, `costA`, `costB`, `result`.
    pub keys: [i32; 4],
}

impl Offer {
    /// `getCostA`: the base cost moved by demand and the special price, within one to a stack.
    pub fn cost_a(&self) -> ItemStack {
        let base = self.base_cost_a.count;
        let demand = floor((base * self.demand) as f32 * self.price_multiplier).max(0);
        let count = (base + demand + self.special_price).clamp(1, self.base_cost_a.max_stack_size());
        self.base_cost_a.with_count(count)
    }

    pub fn is_out_of_stock(&self) -> bool {
        self.uses >= self.max_uses
    }
}

fn can_level_up(level: i32) -> bool {
    (1..5).contains(&level)
}

pub struct Merchant {
    pub base: Base,
    pub offers: Vec<Offer>,
    /// `menu.getTraderLevel()`, `getTraderXp()`, `getFutureTraderXp()`.
    pub level: i32,
    pub xp: i32,
    pub future_xp: i32,
    pub show_progress_bar: bool,
    pub can_restock: bool,
    /// `shopItem`: the selected offer.
    pub shop_item: usize,
    pub scroll_off: usize,
    hovered: [bool; BUTTONS],
}

impl Merchant {
    pub fn new(width: i32, height: i32, title: Text, inventory: Text) -> Merchant {
        let mut slots = vec![Slot::new(0, 136, 37), Slot::new(1, 162, 37), Slot::new(2, 220, 37)];
        menus::standard_inventory(&mut slots, 108, 84);
        let mut base = Base::new(width, height, [276, 166], title, inventory, slots);
        base.inventory_label[0] = 107;
        Merchant { base, offers: Vec::new(), level: 0, xp: 0, future_xp: 0, show_progress_bar: false, can_restock: false, shop_item: 0, scroll_off: 0, hovered: [false; BUTTONS] }
    }

    fn can_scroll(&self) -> bool {
        self.offers.len() > BUTTONS
    }

    /// `mouseClicked`'s own part: the scroller grabbed, or an offer's button (its offer's index;
    /// `postButtonClick` selects it as `shopItem`).
    pub fn mouse_clicked(&self, mouse: [f64; 2]) -> Option<Press> {
        let (xo, yo) = ((self.base.width - self.base.image_width) / 2, (self.base.height - self.base.image_height) / 2);
        let [x, y] = mouse;
        if self.can_scroll() && x > f64::from(xo + 94) && x < f64::from(xo + 100) && y > f64::from(yo + 18) && y <= f64::from(yo + 18 + 140) {
            return Some(Press::Scroller);
        }
        let left = self.base.left + 5;
        (0..BUTTONS.min(self.offers.len()))
            .find(|&i| {
                let top = self.base.top + 18 + 20 * i as i32;
                x >= f64::from(left) && y >= f64::from(top) && x < f64::from(left + 88) && y < f64::from(top + 20)
            })
            .map(|i| Press::Button(i + self.scroll_off))
    }

    /// `mouseDragged` while the scroller is held.
    pub fn mouse_dragged(&mut self, mouse: [f64; 2]) {
        let top = self.base.top + 18;
        let max = self.offers.len() as i32 - BUTTONS as i32;
        let scrolled = ((mouse[1] as f32 - top as f32 - 13.5) / (139.0 - 27.0)) * max as f32 + 0.5;
        self.scroll_off = (scrolled as i32).clamp(0, max.max(0)) as usize;
    }

    /// `mouseScrolled` by `dy` away from any slot.
    pub fn scroll(&mut self, dy: f64) {
        if self.can_scroll() {
            let max = self.offers.len() as i32 - BUTTONS as i32;
            self.scroll_off = ((self.scroll_off as f64 - dy) as i32).clamp(0, max) as usize;
        }
    }

    fn scroller(&self, gui: &mut Gui, xo: i32, yo: i32) {
        let steps = self.offers.len() as i32 + 1 - BUTTONS as i32;
        if steps <= 1 {
            gui.sprite("container/villager/scroller_disabled", xo + 94, yo + 18, 6, 27);
            return;
        }
        let left_over = 139 - (27 + (steps - 1) * 139 / steps);
        let step_height = 1 + left_over / steps + 139 / steps;
        let scroll = self.scroll_off as i32;
        let y = if scroll == steps - 1 { 113 } else { 113.min(scroll * step_height) };
        gui.sprite("container/villager/scroller", xo + 94, yo + 18 + y, 6, 27);
    }

    fn offer(&self, gui: &mut Gui, offer: &Offer, xo: i32, y: i32) {
        let [base_key, cost_key, b_key, result_key] = offer.keys;
        let x = xo + 10;
        let cost = offer.cost_a();
        gui.item(cost_key, x, y, 0);
        let one = |n: i32| (n == 1).then(|| "1".to_owned());
        if offer.base_cost_a.count == cost.count {
            gui.decorations(cost_key, x, y, None);
        } else {
            gui.decorations(base_key, x, y, one(offer.base_cost_a.count));
            gui.decorations(cost_key, x + 14, y, one(cost.count));
            gui.sprite("container/villager/discount_strikethrough", x + 7, y + 12, 9, 2);
        }
        if offer.cost_b.as_ref().is_some_and(|b| !b.is_empty()) {
            gui.item(b_key, xo + 40, y, 0);
            gui.decorations(b_key, xo + 40, y, None);
        }
        let arrow = if offer.is_out_of_stock() { "container/villager/trade_arrow_out_of_stock" } else { "container/villager/trade_arrow" };
        gui.sprite(arrow, xo + 60, y + 3, 10, 9);
        if !offer.result.is_empty() {
            gui.item(result_key, xo + 73, y, 0);
            gui.decorations(result_key, xo + 73, y, None);
        }
    }

    fn progress_bar(&self, gui: &mut Gui, xo: i32, yo: i32) {
        if self.level >= 5 {
            return;
        }
        gui.sprite("container/villager/experience_bar_background", xo + 136, yo + 16, 102, 5);
        if !can_level_up(self.level) {
            return;
        }
        let min = XP_THRESHOLDS[self.level as usize - 1];
        if self.xp < min {
            return;
        }
        let multiplier = 102.0 / (XP_THRESHOLDS[self.level as usize] - min) as f32;
        let w = floor(multiplier * (self.xp - min) as f32).min(102);
        gui.region("container/villager/experience_bar_current", [102, 5], [0, 0], xo + 136, yo + 16, w, 5);
        if self.future_xp > 0 {
            let future = floor(self.future_xp as f32 * multiplier).min(102 - w);
            gui.region("container/villager/experience_bar_result", [102, 5], [w, 0], xo + 136 + w, yo + 16, future, 5);
        }
    }

    /// `TradeOfferButton.extractToolTip`: the hovered column's stack.
    fn button_tooltip(&mut self, i: usize, env: &Env, mouse: [i32; 2]) {
        let Some(offer) = self.offers.get(i + self.scroll_off) else { return };
        let x = self.base.left + 5;
        let stack = if mouse[0] < x + 20 {
            Some(offer.cost_a())
        } else if mouse[0] < x + 50 && mouse[0] > x + 30 {
            offer.cost_b.clone().filter(|b| !b.is_empty())
        } else if mouse[0] > x + 65 {
            Some(offer.result.clone())
        } else {
            None
        };
        if let Some(stack) = stack {
            let t = item_tooltip(self, &stack, env, mouse);
            self.base.tooltip.set(t, false);
        }
    }

    /// The rest of `extractContents` after the abstract screen's.
    fn extras(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2]) {
        if self.offers.is_empty() {
            return;
        }
        let (xo, yo) = (self.base.left, self.base.top);
        self.scroller(gui, xo, yo);
        let mut y = yo + 17;
        for (i, offer) in self.offers.iter().enumerate() {
            if self.can_scroll() && !(self.scroll_off..self.scroll_off + BUTTONS).contains(&i) {
                continue;
            }
            self.offer(gui, offer, xo, y + 2);
            y += 20;
        }
        let selected = &self.offers[self.shop_item];
        if self.show_progress_bar {
            self.progress_bar(gui, xo, yo);
        }
        if selected.is_out_of_stock() && self.hovering(186, 35, 22, 21, mouse) && self.can_restock {
            self.base.tooltip.set(text_tooltip(Text::translate("merchant.deprecated", &[], env.lang), mouse), false);
        }
        for i in 0..BUTTONS {
            if self.hovered[i] {
                self.button_tooltip(i, env, mouse);
            }
        }
    }
}

impl Screen for Merchant {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn background(&mut self, gui: &mut Gui, _env: &Env, _mouse: [i32; 2], _partial: f32) {
        transparent_background(gui);
        let b = &self.base;
        gui.draws.push(GuiDraw::Blit {
            texture: "gui/container/villager".into(),
            blend: Blend::Textured,
            x: b.left,
            y: b.top,
            w: b.image_width,
            h: b.image_height,
            uv: [0.0, 0.0],
            src: [b.image_width, b.image_height],
            tex: [512, 256],
            color: WHITE,
        });
        if self.offers.get(self.shop_item).is_some_and(Offer::is_out_of_stock) {
            gui.sprite("container/villager/out_of_stock", b.left + 83 + 99, b.top + 35, 28, 21);
        }
    }

    /// The seven `Button.Plain`s, those past the last offer hidden.
    fn widgets(&mut self, gui: &mut Gui, _env: &Env, mouse: [i32; 2], _partial: f32) {
        let x = self.base.left + 5;
        for i in 0..BUTTONS.min(self.offers.len()) {
            let y = self.base.top + 18 + 20 * i as i32;
            let hovered = mouse[0] >= x && mouse[1] >= y && mouse[0] < x + 88 && mouse[1] < y + 20;
            self.hovered[i] = hovered;
            gui.sprite(if hovered { "widget/button_highlighted" } else { "widget/button" }, x, y, 88, 20);
            gui.scrolling(Text::default(), (x + 2 + x + 86) / 2, x + 2, x + 86, y, y + 20, WHITE);
        }
    }

    fn labels(&mut self, gui: &mut Gui, env: &Env, _mouse: [i32; 2]) {
        let b = &self.base;
        let title = if self.level > 0 && self.level <= 5 && self.show_progress_bar {
            let level = Text::translate(&format!("merchant.level.{}", self.level), &[], env.lang);
            Text::translate("merchant.title", &[b.title.clone(), level], env.lang)
        } else {
            b.title.clone()
        };
        gui.text(title.clone(), 49 + b.image_width / 2 - env.font.width(&title) / 2, 6, LABEL, false);
        gui.text(b.inventory_title.clone(), b.inventory_label[0], b.inventory_label[1], LABEL, false);
        let trades = Text::translate("merchant.trades", &[], env.lang);
        gui.text(trades.clone(), 5 - env.font.width(&trades) / 2 + 48, 6, LABEL, false);
    }

    fn render(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2], partial: f32) {
        contents(self, gui, env, mouse, partial);
        self.extras(gui, env, mouse);
        carried(self.base(), gui, mouse);
        hovered_tooltip(self, env, mouse);
    }
}
