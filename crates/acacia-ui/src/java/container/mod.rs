//! Container screens as Java 26.3's `AbstractContainerScreen` and its subclasses draw them: the
//! screen's background, its widgets, then under the window's origin its labels, the hovered
//! slot's highlight, every active slot's item, the carried stack and the hovered stack's tooltip.
//! A screen implements [`Screen`] (Java's overridable methods) over a [`Base`] (the abstract
//! class's fields); [`frame`] draws one frame as `extractRenderStateWithTooltipAndSubtitles` does.

pub mod anvil;
pub mod beacon;
pub mod brewing;
pub mod cartography;
pub mod crafter;
pub mod creative;
pub mod creative_search;
pub mod effects;
pub mod enchanting;
pub mod enchanting_names;
pub mod entity;
pub mod grindstone;
pub mod input;
pub mod item_combiner;
pub mod lectern;
pub mod loom;
pub mod menus;
pub mod merchant;
pub mod mount;
pub mod recipe_book;
pub mod simple;
pub mod slot;
pub mod smithing;
pub mod stonecutter;
#[cfg(test)]
mod tests;

pub use slot::{QuickCraft, Slot, can_quick_replace, extract_slot, quick_craft_count, same_item};

use super::gui::{Gui, Metrics};
use super::text::{Lang, Text};
use super::tooltip::game::Game;
use super::tooltip::item::{self, Details, Display};
use super::tooltip::stack::ItemStack;
use super::tooltip::{Deferred, Line, Positioner, Tooltip};

/// The label colour (`-12566464`).
pub const LABEL: u32 = 0xFF40_4040;
/// The draw key of the carried stack (slots are keyed by their menu index).
pub const CARRIED: i32 = -2;
/// With [`Base::distinct_keys`]: the quick-craft stack over slot `i` is keyed `QUICK + i`, the
/// carried remainder `QUICK - 1`.
pub const QUICK: i32 = -1000;

/// What the screens read besides their own state.
pub struct Env<'a> {
    pub font: &'a dyn Metrics,
    pub lang: Lang<'a>,
    /// `options.advancedItemTooltips`.
    pub advanced: bool,
    /// The registry facts tooltips read.
    pub game: &'a Game,
    /// The player has `COMMANDS_GAMEMASTER`.
    pub op: bool,
}

impl Env<'_> {
    /// `getTooltipLines`' context for `stack`, with the local player.
    pub fn details<'s>(&'s self, stack: &ItemStack, creative: bool) -> Details<'s> {
        Details { lang: self.lang, advanced: self.advanced, creative, game: self.game, player: Some(self.game.player(self.op)), key: stack.key }
    }
}

/// `AbstractContainerScreen`'s fields.
#[derive(Clone, Debug, Default)]
pub struct Base {
    pub width: i32,
    pub height: i32,
    pub image_width: i32,
    pub image_height: i32,
    pub left: i32,
    pub top: i32,
    pub title: Text,
    pub inventory_title: Text,
    pub title_label: [i32; 2],
    pub inventory_label: [i32; 2],
    pub slots: Vec<Slot>,
    pub carried: Option<ItemStack>,
    pub quick: Option<QuickCraft>,
    /// Index into `slots`.
    pub hovered: Option<usize>,
    pub tooltip: Deferred,
    /// Key stacks the oracle keys -1 (a quick craft's) apart, for a painter that looks them up.
    pub distinct_keys: bool,
}

impl Base {
    /// The constructor and `init`: centred, labels at their defaults.
    pub fn new(width: i32, height: i32, image: [i32; 2], title: Text, inventory_title: Text, slots: Vec<Slot>) -> Base {
        Base {
            width,
            height,
            image_width: image[0],
            image_height: image[1],
            left: (width - image[0]) / 2,
            top: (height - image[1]) / 2,
            title,
            inventory_title,
            title_label: [8, 6],
            inventory_label: [8, image[1] - 94],
            slots,
            ..Base::default()
        }
    }

    pub fn carried(&self) -> Option<&ItemStack> {
        self.carried.as_ref().filter(|s| !s.is_empty())
    }

    /// `recalculateQuickCraftRemaining`.
    pub fn recalculate_quick_craft(&mut self) {
        let (Some(carried), Some(quick)) = (self.carried.clone().filter(|s| !s.is_empty()), self.quick.as_mut()) else { return };
        if quick.kind == 2 {
            quick.remainder = carried.max_stack_size();
            return;
        }
        quick.remainder = carried.count;
        for &i in &quick.slots {
            let slot = &self.slots[i];
            let carry = slot.item.as_ref().filter(|s| !s.is_empty()).map_or(0, |s| s.count);
            let max = slot.max_stack.min(carried.max_stack_size());
            let count = (quick_craft_count(quick.slots.len(), quick.kind, &carried) + carry).min(max);
            quick.remainder -= count - carry;
        }
    }
}

/// `AbstractContainerScreen`'s overridable methods; the defaults are the abstract class's.
pub trait Screen {
    fn base(&self) -> &Base;
    fn base_mut(&mut self) -> &mut Base;

    /// `extractBackground`: the in-game UI's darkening, then the subclass's window.
    fn background(&mut self, gui: &mut Gui, _env: &Env, _mouse: [i32; 2], _partial: f32) {
        transparent_background(gui);
    }

    /// `Screen.extractRenderState`: the widgets (`renderables`) in order.
    fn widgets(&mut self, _gui: &mut Gui, _env: &Env, _mouse: [i32; 2], _partial: f32) {}

    /// `extractLabels`, under the window's origin.
    fn labels(&mut self, gui: &mut Gui, _env: &Env, _mouse: [i32; 2]) {
        let b = self.base();
        gui.text(b.title.clone(), b.title_label[0], b.title_label[1], LABEL, false);
        gui.text(b.inventory_title.clone(), b.inventory_label[0], b.inventory_label[1], LABEL, false);
    }

    /// `extractSlots`.
    fn slots(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2]) {
        for i in 0..self.base().slots.len() {
            if self.base().slots[i].active {
                self.slot(gui, env, i, mouse);
            }
        }
    }

    /// `extractSlot` of `slots[i]`.
    fn slot(&mut self, gui: &mut Gui, env: &Env, i: usize, _mouse: [i32; 2]) {
        extract_slot(self.base_mut(), gui, env, i);
    }

    /// `hasClickedOutside`: off the window.
    fn clicked_outside(&self, mouse: [f64; 2]) -> bool {
        let b = self.base();
        let (x, y) = (mouse[0], mouse[1]);
        x < f64::from(b.left) || y < f64::from(b.top) || x >= f64::from(b.left + b.image_width) || y >= f64::from(b.top + b.image_height)
    }

    /// `isHovering(left, top, w, h, x, y)` in window coordinates.
    fn hovering(&self, x: i32, y: i32, w: i32, h: i32, mouse: [i32; 2]) -> bool {
        let b = self.base();
        let (mx, my) = (mouse[0] - b.left, mouse[1] - b.top);
        mx >= x - 1 && mx < x + w + 1 && my >= y - 1 && my < y + h + 1
    }

    /// `getTooltipFromContainerItem`.
    fn tooltip_lines(&self, stack: &ItemStack, env: &Env) -> Vec<Text> {
        item::tooltip_lines(stack, &env.details(stack, false))
    }

    /// `extractRenderState`: contents, carried stack, hovered tooltip.
    fn render(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2], partial: f32) {
        contents(self, gui, env, mouse, partial);
        carried(self.base(), gui, mouse);
        hovered_tooltip(self, env, mouse);
    }
}

/// `blit(GUI_TEXTURED, texture, x, y, u, v, w, h, 256, 256)`; `texture` under `textures/`, no `.png`.
pub fn blit(gui: &mut Gui, texture: &'static str, x: i32, y: i32, uv: [f32; 2], w: i32, h: i32) {
    gui.draws.push(super::gui::GuiDraw::Blit {
        texture: texture.into(),
        blend: super::gui::Blend::Textured,
        x,
        y,
        w,
        h,
        uv,
        src: [w, h],
        tex: [256, 256],
        color: super::mth::WHITE,
    });
}

/// `Screen.extractTransparentBackground`.
pub fn transparent_background(gui: &mut Gui) {
    gui.gradient(0, 0, gui.width, gui.height, 0xC010_1010, 0xD010_1010);
}

/// `extractRenderStateWithTooltipAndSubtitles`: background, contents, the deferred tooltip.
pub fn frame(s: &mut dyn Screen, gui: &mut Gui, env: &Env, mouse: [i32; 2], partial: f32) {
    gui.next_stratum();
    s.background(gui, env, mouse, partial);
    gui.next_stratum();
    s.render(gui, env, mouse, partial);
    s.base_mut().tooltip.extract(gui, env.font);
}

/// `getHoveredSlot`.
pub fn hovered_slot<S: Screen + ?Sized>(s: &S, mouse: [i32; 2]) -> Option<usize> {
    s.base().slots.iter().position(|slot| slot.active && s.hovering(slot.x, slot.y, 16, 16, mouse))
}

/// `extractContents`.
pub fn contents<S: Screen + ?Sized>(s: &mut S, gui: &mut Gui, env: &Env, mouse: [i32; 2], partial: f32) {
    s.widgets(gui, env, mouse, partial);
    let (left, top) = (s.base().left, s.base().top);
    gui.push(vec![super::gui::PoseOp::Translate(left as f32, top as f32)]);
    s.labels(gui, env, mouse);
    let hovered = hovered_slot(s, mouse);
    s.base_mut().hovered = hovered;
    highlight(s.base(), gui, "container/slot_highlight_back");
    s.slots(gui, env, mouse);
    highlight(s.base(), gui, "container/slot_highlight_front");
    gui.pop();
}

fn highlight(b: &Base, gui: &mut Gui, sprite: &'static str) {
    if let Some(slot) = b.hovered.map(|i| &b.slots[i]).filter(|s| s.highlightable) {
        gui.sprite(sprite, slot.x - 4, slot.y - 4, 24, 24);
    }
}

/// `extractCarriedItem`.
pub fn carried(b: &Base, gui: &mut Gui, mouse: [i32; 2]) {
    let Some(carried) = b.carried() else { return };
    let (mut key, mut stack, mut count) = (CARRIED, carried.clone(), None);
    if let Some(quick) = b.quick.as_ref().filter(|q| q.slots.len() > 1) {
        key = if b.distinct_keys { QUICK - 1 } else { -1 };
        stack = carried.with_count(quick.remainder);
        if stack.is_empty() {
            count = Some("§e0".to_owned());
        }
    }
    gui.next_stratum();
    let (x, y) = (mouse[0] - 8, mouse[1] - 8);
    if !stack.is_empty() {
        gui.item(key, x, y, 0);
        gui.decorations(key, x, y, count);
    }
}

/// `extractTooltip`: the hovered stack's tooltip unless a stack is carried.
pub fn hovered_tooltip<S: Screen + ?Sized>(s: &mut S, env: &Env, mouse: [i32; 2]) {
    let b = s.base();
    let Some(stack) = b.hovered.and_then(|i| b.slots[i].item.as_ref()).filter(|s| !s.is_empty()) else { return };
    // TODO: showTooltipWithItemInHand (a bundle's tooltip shows over a carried stack).
    if b.carried().is_some() {
        return;
    }
    let t = item_tooltip(s, stack, env, mouse);
    s.base_mut().tooltip.set(t, false);
}

/// `setTooltipForNextFrame(font, stack, x, y)`: the stack's tooltip lines with its image second,
/// its style, a gap after the name (`tooltip::bundle::stack_tooltip` over the screen's lines).
pub fn item_tooltip<S: Screen + ?Sized>(s: &S, stack: &ItemStack, env: &Env, mouse: [i32; 2]) -> Tooltip {
    let mut lines: Vec<Line> = s.tooltip_lines(stack, env).into_iter().map(Line::Text).collect();
    if let Some(image) = super::tooltip::bundle::image(stack, &Display::of(stack), env.font, env.lang, env.game) {
        lines.insert(lines.len().min(1), image);
    }
    let style = stack.get("tooltip_style").and_then(|v| v.as_str()).map(str::to_owned);
    Tooltip { lines, x: mouse[0], y: mouse[1], positioner: Positioner::Default, style, extra_space: true }
}

/// `setTooltipForNextFrame(component, x, y)`: one plain line.
pub fn text_tooltip(text: Text, mouse: [i32; 2]) -> Tooltip {
    Tooltip { lines: vec![Line::Text(text)], x: mouse[0], y: mouse[1], positioner: Positioner::Default, style: None, extra_space: false }
}
