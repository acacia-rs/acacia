//! Container screens as Java 26.3's `AbstractContainerScreen` and its subclasses draw them: the
//! screen's background, its widgets, then under the window's origin its labels, the hovered
//! slot's highlight, every active slot's item, the carried stack and the hovered stack's tooltip.
//! A screen implements [`Screen`] (Java's overridable methods) over a [`Base`] (the abstract
//! class's fields); [`frame`] draws one frame as `extractRenderStateWithTooltipAndSubtitles` does.

pub mod brewing;
pub mod crafter;
pub mod menus;
pub mod simple;

use super::gui::{Gui, Metrics};
use super::text::{Lang, Text};
use super::tooltip::item::{self, Context};
use super::tooltip::stack::ItemStack;
use super::tooltip::{Deferred, Line, Positioner, Tooltip};

/// The label colour (`-12566464`).
pub const LABEL: u32 = 0xFF40_4040;
/// The draw key of the carried stack (slots are keyed by their menu index).
pub const CARRIED: i32 = -2;

/// What the screens read besides their own state.
pub struct Env<'a> {
    pub font: &'a dyn Metrics,
    pub lang: Lang<'a>,
    /// `options.advancedItemTooltips`.
    pub advanced: bool,
}

/// A menu `Slot` and what the screen asks of it.
#[derive(Clone, Debug, Default)]
pub struct Slot {
    /// `slot.index`, the draw key of its stack.
    pub index: usize,
    pub x: i32,
    pub y: i32,
    pub item: Option<ItemStack>,
    pub active: bool,
    pub highlightable: bool,
    pub fake: bool,
    /// `getNoItemIcon`.
    pub icon: Option<&'static str>,
    /// `getMaxStackSize()`: its container's limit.
    pub max_stack: i32,
    /// `menu.canDragTo(slot)`.
    pub drag_target: bool,
}

impl Slot {
    pub fn new(index: usize, x: i32, y: i32) -> Slot {
        Slot { index, x, y, item: None, active: true, highlightable: true, fake: false, icon: None, max_stack: 99, drag_target: true }
    }

    pub fn has_item(&self) -> bool {
        self.item.as_ref().is_some_and(|s| !s.is_empty())
    }
}

/// `isSameItemSameComponents`.
pub fn same_item(a: &ItemStack, b: &ItemStack) -> bool {
    a.id == b.id && a.components == b.components
}

/// `AbstractContainerMenu.canItemQuickReplace(slot, stack, true)`.
pub fn can_quick_replace(slot: &Slot, stack: &ItemStack) -> bool {
    match slot.item.as_ref().filter(|s| !s.is_empty()) {
        Some(item) if same_item(stack, item) => item.count <= stack.max_stack_size(),
        Some(_) => false,
        None => true,
    }
}

/// `getQuickCraftPlaceCount`.
pub fn quick_craft_count(slots: usize, kind: i32, stack: &ItemStack) -> i32 {
    match kind {
        0 => super::mth::floor(stack.count as f32 / slots as f32),
        1 => 1,
        2 => stack.max_stack_size(),
        _ => stack.count,
    }
}

/// A quick-craft drag in progress: `quickCraftingType` and the slots dragged over (indices into
/// `Base::slots`, in the set's order).
#[derive(Clone, Debug, Default)]
pub struct QuickCraft {
    pub kind: i32,
    pub slots: Vec<usize>,
    pub remainder: i32,
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

    /// `isHovering(left, top, w, h, x, y)` in window coordinates.
    fn hovering(&self, x: i32, y: i32, w: i32, h: i32, mouse: [i32; 2]) -> bool {
        let b = self.base();
        let (mx, my) = (mouse[0] - b.left, mouse[1] - b.top);
        mx >= x - 1 && mx < x + w + 1 && my >= y - 1 && my < y + h + 1
    }

    /// `getTooltipFromContainerItem`.
    fn tooltip_lines(&self, stack: &ItemStack, env: &Env) -> Vec<Text> {
        item::lines(stack, &Context { lang: env.lang, advanced: env.advanced, creative: false })
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
        key = -1;
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
    let lines = s.tooltip_lines(stack, env).into_iter().map(Line::Text).collect();
    let style = stack.get("tooltip_style").and_then(|v| v.as_str()).map(str::to_owned);
    let t = Tooltip { lines, x: mouse[0], y: mouse[1], positioner: Positioner::Default, style, extra_space: true };
    s.base_mut().tooltip.set(t, false);
}

/// `extractSlot` of `slots[i]`.
pub fn extract_slot(b: &mut Base, gui: &mut Gui, _env: &Env, i: usize) {
    let slot = &b.slots[i];
    let (x, y) = (slot.x, slot.y);
    let mut stack = slot.item.clone().filter(|s| !s.is_empty());
    let mut key = slot.index as i32;
    let mut quick_stack = false;
    let mut count = None;
    if let (Some(quick), Some(carried)) = (b.quick.as_ref(), b.carried()) {
        if quick.slots.contains(&i) {
            if quick.slots.len() == 1 {
                return;
            }
            if can_quick_replace(slot, carried) && slot.drag_target {
                quick_stack = true;
                let max = carried.max_stack_size().min(slot.max_stack);
                let carry = stack.as_ref().map_or(0, |s| s.count);
                let mut n = quick_craft_count(quick.slots.len(), quick.kind, carried) + carry;
                if n > max {
                    n = max;
                    count = Some(format!("§e{max}"));
                }
                stack = Some(carried.with_count(n)).filter(|s| !s.is_empty());
                key = -1;
            } else {
                let q = b.quick.as_mut().expect("checked");
                q.slots.retain(|&s| s != i);
                b.recalculate_quick_craft();
            }
        }
    }
    let slot = &b.slots[i];
    if stack.is_none() && slot.active {
        if let Some(icon) = slot.icon {
            gui.sprite(icon, x, y, 16, 16);
            return;
        }
    }
    if quick_stack {
        gui.fill(x, y, x + 16, y + 16, 0x80FF_FFFF);
    }
    if stack.is_some() {
        gui.item(key, x, y, slot.x + slot.y * b.image_width);
        gui.decorations(key, x, y, count);
    }
}
