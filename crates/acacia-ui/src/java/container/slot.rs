//! Menu slots and the quick-craft drag over them: `Slot`, the menu's quick-craft rules and
//! `AbstractContainerScreen.extractSlot`.

use super::{Base, Env};
use crate::java::gui::Gui;
use crate::java::mth::floor;
use crate::java::tooltip::stack::ItemStack;

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
        0 => floor(stack.count as f32 / slots as f32),
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
