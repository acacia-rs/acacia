//! Clicks in the open menu (the own inventory, window 0, or a container screen), predicted as
//! `AbstractContainerMenu.doClick` does and sent as `MultiPlayerGameMode.handleContainerInput` does: the slots that
//! changed and the carried stack go with the click, hashed. Not modelled: crafting and furnace result slots, slot
//! filters (armour and brewing slots take whatever is put there), and the offhand key inside container screens.

use acacia_java_proto::VarInt;
use acacia_java_proto::container::{ContainerClick, ContainerClose, ContainerInput, HashedStack};
use acacia_java_proto::ids::play::serverbound as to;
use acacia_java_proto::item::ItemStack;
use acacia_registry::Item;

use crate::frame::Error;
use crate::inventory::{ARMOUR_HEAD, Inventory, OFFHAND};
use crate::join::Client;

/// `slotIndex` of a click outside the screen.
pub const OUTSIDE: i16 = -999;
/// `Inventory.SLOT_OFFHAND`, the SWAP button for the offhand key.
pub const OFFHAND_BUTTON: i8 = 40;
/// `Container.getMaxStackSize()` of the player's inventory and of block containers.
const SLOT_MAX: i32 = 99;
const HOTBAR_SIZE: usize = 9;

/// One click: a menu slot (or [`OUTSIDE`]), a button and the input it is.
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

    /// Shift-click on `slot`.
    pub fn quick_move(slot: i16) -> Self {
        Click { slot, button: 0, input: ContainerInput::QuickMove }
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

/// The open menu's slots and carried stack, copied out to be clicked on.
struct Menu {
    slots: Vec<Option<ItemStack>>,
    cursor: Option<ItemStack>,
    /// Window 0 only: armour slots hold one, and the offhand key reaches the offhand.
    own: bool,
    hotbar: usize,
}

impl Menu {
    /// `Slot.getMaxStackSize(stack)`.
    fn slot_max(&self, slot: usize, stack: &ItemStack) -> i32 {
        let armour = self.own && (ARMOUR_HEAD..ARMOUR_HEAD + 4).contains(&slot);
        (if armour { 1 } else { SLOT_MAX }).min(max_stack(stack))
    }

    /// `Slot.safeInsert`: puts up to `amount` of `carried` into `slot`, leaving the rest carried.
    fn safe_insert(&mut self, slot: usize, carried: &mut Option<ItemStack>, amount: i32) {
        let Some(stack) = carried.as_ref() else { return };
        let current = self.slots[slot].as_ref().map_or(0, |s| s.count);
        let n = amount.min(stack.count).min(self.slot_max(slot, stack) - current);
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
    fn click(&mut self, Click { slot, button, input }: Click) {
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
        let Some(i) = usize::try_from(slot).ok().filter(|&i| i < self.slots.len()) else { return };
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
                if c.count <= self.slot_max(i, c) {
                    std::mem::swap(&mut self.slots[i], &mut carried);
                }
            }
        }
        self.cursor = carried;
    }

    /// The overflow branch (a source too big for the target, which only armour slots cause) is not modelled.
    fn swap(&mut self, i: usize, button: i8) {
        let source = match button {
            0..9 => self.hotbar + button as usize,
            OFFHAND_BUTTON if self.own => OFFHAND,
            _ => return,
        };
        if self.slots[source].as_ref().is_some_and(|s| s.count > self.slot_max(i, s)) {
            return;
        }
        self.slots.swap(source, i);
    }
}

impl Inventory {
    fn menu(&self) -> Menu {
        let len = self.menu_len();
        Menu {
            slots: (0..len).map(|i| self.menu_slot(i).cloned()).collect(),
            cursor: self.cursor.clone(),
            own: self.window.is_none(),
            hotbar: len - HOTBAR_SIZE - usize::from(self.window.is_none()),
        }
    }

    /// Applies `click` to the open menu and returns the packet that tells the server about it.
    pub fn click(&mut self, click: Click) -> ContainerClick {
        let mut menu = self.menu();
        let before = menu.slots.clone();
        menu.click(click);
        let mut changed = Vec::new();
        self.predicted.clear();
        for (i, (old, new)) in before.into_iter().zip(menu.slots).enumerate() {
            if old != new {
                changed.push((i as i16, new.as_ref().map(|s| HashedStack::of(s, &self.enchantments))));
                if let Some(at) = self.locate(i) {
                    if let crate::window::At::Player(p) = at {
                        self.predicted.push(p);
                    }
                    self.put(at, new);
                }
            }
        }
        self.cursor = menu.cursor;
        let (container, state_id) = self.menu_ids();
        let carried = self.cursor.as_ref().map(|s| HashedStack::of(s, &self.enchantments));
        ContainerClick { container, state_id, slot: click.slot, button: click.button, input: click.input, changed, carried }
    }

    /// `click` as a packet that predicts nothing, for clicks the server alone can work out (taking a crafting result):
    /// every slot it changes comes back from the server.
    pub fn unpredicted(&self, click: Click) -> ContainerClick {
        let (container, state_id) = self.menu_ids();
        let carried = self.cursor.as_ref().map(|s| HashedStack::of(s, &self.enchantments));
        ContainerClick { container, state_id, slot: click.slot, button: click.button, input: click.input, changed: Vec::new(), carried }
    }
}

impl Client {
    /// Predicts `click` in the open menu and queues it for the next [`Client::step`].
    pub fn click(&mut self, click: Click) -> Result<(), Error> {
        let packet = self.inventory.click(click);
        self.queue(to::CONTAINER_CLICK, &packet)
    }

    /// Queues `click` without predicting it (see [`Inventory::unpredicted`]).
    pub fn click_unpredicted(&mut self, click: Click) -> Result<(), Error> {
        let packet = self.inventory.unpredicted(click);
        self.queue(to::CONTAINER_CLICK, &packet)
    }

    /// Closes the open screen, as vanilla does when it closes; queued like clicks. The carried stack is the
    /// server's to put back or drop.
    pub fn close_screen(&mut self) -> Result<(), Error> {
        let (container, _) = self.inventory.menu_ids();
        self.inventory.window = None;
        self.queue(to::CONTAINER_CLOSE, &ContainerClose { container: VarInt(container) })
    }
}

#[cfg(test)]
#[path = "clicks_tests.rs"]
mod tests;
