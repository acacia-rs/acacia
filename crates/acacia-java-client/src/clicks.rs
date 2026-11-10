//! Clicks in the player's own inventory (window 0), predicted as `AbstractContainerMenu.doClick` does and sent as
//! `MultiPlayerGameMode.handleContainerInput` does: the slots that changed and the carried stack go with the click,
//! hashed. Crafting slots are not modelled; armour slots take whatever is put there.

use acacia_java_proto::container::{ContainerClick, ContainerClose, ContainerInput, HashedStack};
use acacia_java_proto::ids::play::serverbound as to;
use acacia_java_proto::inventory::PLAYER_CONTAINER;
use acacia_java_proto::item::ItemStack;
use acacia_java_proto::VarInt;
use acacia_registry::Item;

use crate::frame::Error;
use crate::inventory::{ARMOUR_HEAD, Inventory, SLOTS, window_slot};
use crate::join::Client;

/// `slotIndex` of a click outside the screen.
pub const OUTSIDE: i16 = -999;
/// `Inventory.SLOT_OFFHAND`, the SWAP button for the offhand key.
pub const OFFHAND_BUTTON: i8 = 40;
/// `Container.getMaxStackSize()` of the player's inventory.
const SLOT_MAX: i32 = 99;

/// One click: a window slot (or [`OUTSIDE`]), a button and the input it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Click {
    pub slot: i16,
    pub button: i8,
    pub input: ContainerInput,
}

impl Click {
    pub fn left(slot: i16) -> Self {
        Click { slot, button: 0, input: ContainerInput::Pickup }
    }

    pub fn right(slot: i16) -> Self {
        Click { slot, button: 1, input: ContainerInput::Pickup }
    }

    /// The number key `button` (hotbar 0-8, or [`OFFHAND_BUTTON`]) over `slot`.
    pub fn swap(slot: i16, button: i8) -> Self {
        Click { slot, button, input: ContainerInput::Swap }
    }

    /// Q over `slot`; `all` is ctrl+Q.
    pub fn throw(slot: i16, all: bool) -> Self {
        Click { slot, button: i8::from(all), input: ContainerInput::Throw }
    }
}

/// `ItemStack.getMaxStackSize`: the component if the stack sets it, else the item's default.
pub fn max_stack(stack: &ItemStack) -> i32 {
    stack.components.max_stack_size.unwrap_or_else(|| {
        u16::try_from(stack.item).ok().and_then(Item::from_java_id).map_or(64, |i| i32::from(i.max_stack()))
    })
}

/// `Slot.getMaxStackSize(stack)`; armour slots hold one.
fn slot_max(slot: usize, stack: &ItemStack) -> i32 {
    let slot_max = if (ARMOUR_HEAD..ARMOUR_HEAD + 4).contains(&slot) { 1 } else { SLOT_MAX };
    slot_max.min(max_stack(stack))
}

fn same_item(a: &ItemStack, b: &ItemStack) -> bool {
    a.item == b.item && a.components == b.components
}

/// Takes `n` off `stack`, emptying it at zero.
fn split(stack: &mut Option<ItemStack>, n: i32) -> Option<ItemStack> {
    let s = stack.as_mut()?;
    let n = n.min(s.count);
    let taken = ItemStack { count: n, ..s.clone() };
    s.count -= n;
    if s.count <= 0 {
        *stack = None;
    }
    (n > 0).then_some(taken)
}

impl Inventory {
    /// `Slot.safeInsert`: puts up to `amount` of `carried` into `slot`, leaving the rest carried.
    fn safe_insert(&mut self, slot: usize, carried: &mut Option<ItemStack>, amount: i32) {
        let Some(stack) = carried.as_ref() else { return };
        let current = self.slots[slot].as_ref().map_or(0, |s| s.count);
        let n = amount.min(stack.count).min(slot_max(slot, stack) - current);
        if n <= 0 || self.slots[slot].as_ref().is_some_and(|s| !same_item(s, stack)) {
            return;
        }
        let moved = split(carried, n).expect("n > 0");
        match &mut self.slots[slot] {
            Some(s) => s.count += moved.count,
            empty => *empty = Some(moved),
        }
    }

    /// `doClick` for PICKUP, SWAP, THROW and clicks outside; other inputs change nothing.
    fn apply_click(&mut self, click: Click) {
        let Click { slot, button, input } = click;
        if slot == OUTSIDE {
            if input == ContainerInput::Pickup {
                let mut carried = self.cursor.take();
                if button == 1 {
                    split(&mut carried, 1);
                    self.cursor = carried;
                }
            }
            return;
        }
        let Some(i) = usize::try_from(slot).ok().filter(|&i| i < SLOTS) else { return };
        match input {
            ContainerInput::Pickup => self.pickup(i, button == 0),
            ContainerInput::Swap => self.swap(i, button),
            ContainerInput::Throw if self.cursor.is_none() => {
                let amount = if button == 0 { 1 } else { i32::MAX };
                split(&mut self.slots[i], amount);
            }
            _ => {}
        }
    }

    fn pickup(&mut self, i: usize, primary: bool) {
        let mut carried = self.cursor.take();
        match (&self.slots[i], &carried) {
            (None, None) => {}
            (None, Some(c)) => {
                let amount = if primary { c.count } else { 1 };
                self.safe_insert(i, &mut carried, amount);
            }
            (Some(s), None) => {
                let amount = if primary { s.count } else { (s.count + 1) / 2 };
                carried = split(&mut self.slots[i], amount);
            }
            (Some(s), Some(c)) if same_item(s, c) => {
                let amount = if primary { c.count } else { 1 };
                self.safe_insert(i, &mut carried, amount);
            }
            (Some(_), Some(c)) => {
                if c.count <= slot_max(i, c) {
                    std::mem::swap(&mut self.slots[i], &mut carried);
                }
            }
        }
        self.cursor = carried;
    }

    /// The overflow branch (a source too big for the target, which only armour slots cause) is not modelled.
    fn swap(&mut self, i: usize, button: i8) {
        let Some(source) = window_slot(i32::from(button)).filter(|_| (0..9).contains(&button) || button == OFFHAND_BUTTON) else { return };
        let (from, to) = (self.slots[source].clone(), self.slots[i].clone());
        if from.as_ref().is_some_and(|s| s.count > slot_max(i, s)) {
            return;
        }
        self.slots[source] = to;
        self.slots[i] = from;
    }

    /// Applies `click` and returns the packet that tells the server about it.
    pub fn click(&mut self, click: Click) -> ContainerClick {
        let before = self.slots.clone();
        self.apply_click(click);
        let changed = (0..SLOTS)
            .filter(|&i| before[i] != self.slots[i])
            .map(|i| (i as i16, self.slots[i].as_ref().map(HashedStack::of)))
            .collect();
        ContainerClick {
            container: PLAYER_CONTAINER,
            state_id: self.state_id(),
            slot: click.slot,
            button: click.button,
            input: click.input,
            changed,
            carried: self.cursor.as_ref().map(HashedStack::of),
        }
    }
}

impl Client {
    /// Predicts `click` in the own inventory and queues it for the next [`Client::step`].
    pub fn click(&mut self, click: Click) -> Result<(), Error> {
        let packet = self.inventory.click(click);
        self.queue(to::CONTAINER_CLICK, &packet)
    }

    /// Closes the own inventory, as vanilla does when its screen closes; queued like clicks. The carried stack is
    /// the server's to put back or drop.
    pub fn close_inventory(&mut self) -> Result<(), Error> {
        self.queue(to::CONTAINER_CLOSE, &ContainerClose { container: VarInt(PLAYER_CONTAINER) })
    }
}

#[cfg(test)]
#[path = "clicks_tests.rs"]
mod tests;
