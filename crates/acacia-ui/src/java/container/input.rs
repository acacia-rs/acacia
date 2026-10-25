//! `AbstractContainerScreen`'s mouse and key handling over its slots: what a press, a drag, a
//! release or a key asks of the menu (`slotClicked` with its `ContainerInput`), and the
//! quick-craft drag it tracks between them. A screen's own widgets answer first (its
//! `mouse_clicked`); what they leave comes here.

use super::{QuickCraft, Screen, can_quick_replace, hovered_slot};
use crate::java::mth::floor_d;
use crate::java::tooltip::stack::ItemStack;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}

impl Button {
    /// The menu's button number.
    pub fn num(self) -> i32 {
        match self {
            Button::Left => 0,
            Button::Right => 1,
            Button::Middle => 2,
        }
    }
}

/// `ContainerInput`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Pickup,
    QuickMove,
    Swap,
    Clone,
    Throw,
    PickupAll,
}

/// One `slotClicked`, or a finished quick-craft drag (`quickCraftToSlots`).
#[derive(Clone, Debug, PartialEq)]
pub enum Click {
    /// `slot` indexes `Base::slots`; `outside` is the window's slot id -999.
    Slot { slot: Option<usize>, outside: bool, button: i32, input: Input },
    /// Quick-craft `kind` (0 split, 1 one each, 2 a full stack each) over `slots`.
    QuickCraft { kind: i32, slots: Vec<usize> },
}

/// What a press on a list screen's own widgets did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// The list's button of this index (`clickMenuButton`'s id, or the offer).
    Button(usize),
    /// The scroller was grabbed: drags move it until the release.
    Scroller,
}

/// `(y - top - half) / (span - 2 half)`, clamped: a scroller dragged to `y`.
pub fn drag_scroll(y: f64, top: i32, span: i32, half: f32) -> f32 {
    ((y as f32 - top as f32 - half) / (span as f32 - 2.0 * half)).clamp(0.0, 1.0)
}

/// A key the screen handles over the hovered slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// `keyHotbarSlots[i]`.
    Hotbar(i32),
    SwapOffhand,
    /// `keyDrop`; with Control the whole stack.
    Drop { all: bool },
    Pick,
}

/// The screen's pointer state between events.
#[derive(Clone, Debug, Default)]
pub struct Pointer {
    pub quick_crafting: bool,
    pub quick_button: Option<Button>,
    pub quick_kind: i32,
    /// Indices into `Base::slots`, in the order dragged over.
    pub quick_slots: Vec<usize>,
    pub skip_next_release: bool,
    pub double_click: bool,
    pub last_click_slot: Option<usize>,
    pub last_quick_moved: Option<ItemStack>,
}

impl Pointer {
    /// The drag as `extractSlot` and `extractCarriedItem` read it.
    pub fn quick(&self) -> Option<QuickCraft> {
        self.quick_crafting.then(|| QuickCraft { kind: self.quick_kind, slots: self.quick_slots.clone(), remainder: 0 })
    }
}

fn at(mouse: [f64; 2]) -> [i32; 2] {
    [floor_d(mouse[0]), floor_d(mouse[1])]
}

/// The slot id the menu is told: the slot, -999 off the window, -1 for neither.
fn target<S: Screen + ?Sized>(s: &S, mouse: [f64; 2]) -> (Option<usize>, bool) {
    (hovered_slot(s, at(mouse)), s.clicked_outside(mouse))
}

/// `mouseClicked` after the widgets; `cloning` is the pick key on a player with infinite materials.
pub fn mouse_clicked<S: Screen + ?Sized>(s: &S, p: &mut Pointer, mouse: [f64; 2], button: Button, shift: bool, double: bool, cloning: bool) -> Vec<Click> {
    let (slot, outside) = target(s, mouse);
    p.double_click = p.last_click_slot == slot && double;
    p.skip_next_release = false;
    let mut out = Vec::new();
    // Otherwise checkHotbarMouseClicked: hotbar keys bound to mouse buttons, which are not bound here.
    if matches!(button, Button::Left | Button::Right) || cloning {
        if (slot.is_some() || outside) && !p.quick_crafting {
            if s.base().carried().is_none() {
                let input = if cloning {
                    Input::Clone
                } else if !outside && shift {
                    p.last_quick_moved = slot.and_then(|i| s.base().slots[i].item.clone()).filter(|s| !s.is_empty());
                    Input::QuickMove
                } else if outside {
                    Input::Throw
                } else {
                    Input::Pickup
                };
                out.push(Click::Slot { slot: slot.filter(|_| !outside), outside, button: button.num(), input });
                p.skip_next_release = true;
            } else {
                p.quick_crafting = true;
                p.quick_button = Some(button);
                p.quick_slots.clear();
                match button {
                    Button::Left => p.quick_kind = 0,
                    Button::Right => p.quick_kind = 1,
                    Button::Middle if cloning => p.quick_kind = 2,
                    Button::Middle => {}
                }
            }
        }
    }
    p.last_click_slot = slot;
    out
}

/// `mouseDragged`: the slot under the mouse joins the quick craft if it can take the carried stack.
pub fn mouse_dragged<S: Screen + ?Sized>(s: &S, p: &mut Pointer, mouse: [f64; 2]) {
    let Some(i) = hovered_slot(s, at(mouse)) else { return };
    let Some(carried) = s.base().carried() else { return };
    let slot = &s.base().slots[i];
    let room = carried.count as usize > p.quick_slots.len() || p.quick_kind == 2;
    if p.quick_crafting && room && can_quick_replace(slot, carried) && slot.drag_target && !p.quick_slots.contains(&i) {
        p.quick_slots.push(i);
    }
}

/// `mouseReleased`.
pub fn mouse_released<S: Screen + ?Sized>(s: &S, p: &mut Pointer, mouse: [f64; 2], button: Button, shift: bool, cloning: bool) -> Vec<Click> {
    let (slot, outside) = target(s, mouse);
    let mut out = Vec::new();
    // canTakeItemForPickAll: the menus here refuse exactly the slots they refuse drags to.
    if p.double_click && button == Button::Left && slot.is_some_and(|i| s.base().slots[i].drag_target) {
        // TODO: Shift's variant quick-moves every stack like the last one from the slot's container, which Slot doesn't name.
        if !shift {
            out.push(Click::Slot { slot, outside: false, button: 0, input: Input::PickupAll });
        }
        p.double_click = false;
    } else {
        if p.quick_crafting && p.quick_button != Some(button) {
            p.quick_crafting = false;
            p.quick_slots.clear();
            p.skip_next_release = true;
            return out;
        }
        if p.skip_next_release {
            p.skip_next_release = false;
            return out;
        }
        if p.quick_crafting && !p.quick_slots.is_empty() {
            out.push(Click::QuickCraft { kind: p.quick_kind, slots: p.quick_slots.clone() });
        } else if s.base().carried().is_some() {
            let quick = !outside && shift && !cloning;
            if quick {
                p.last_quick_moved = slot.and_then(|i| s.base().slots[i].item.clone()).filter(|s| !s.is_empty());
            }
            let input = if cloning {
                Input::Clone
            } else if quick {
                Input::QuickMove
            } else {
                Input::Pickup
            };
            out.push(Click::Slot { slot: slot.filter(|_| !outside), outside, button: button.num(), input });
        }
    }
    p.quick_crafting = false;
    out
}

/// `keyPressed`'s slot keys over the slot under the mouse.
pub fn key_pressed<S: Screen + ?Sized>(s: &S, mouse: [f64; 2], key: Key) -> Option<Click> {
    let slot = hovered_slot(s, at(mouse))?;
    let click = |button, input| Some(Click::Slot { slot: Some(slot), outside: false, button, input });
    let empty_hand = s.base().carried().is_none();
    match key {
        Key::SwapOffhand if empty_hand => return click(40, Input::Swap),
        Key::Hotbar(i) if empty_hand => return click(i, Input::Swap),
        _ => {}
    }
    if !s.base().slots[slot].has_item() {
        return None;
    }
    match key {
        Key::Pick => click(0, Input::Clone),
        Key::Drop { all } => click(i32::from(all), Input::Throw),
        _ => None,
    }
}
