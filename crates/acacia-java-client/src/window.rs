//! A container screen the server opened (chest, furnace, ...). Its menu lists the container's own slots, then the
//! player's 27 main slots and 9 hotbar slots, which are the player's inventory itself.

use acacia_java_proto::container::OpenScreen;
use acacia_java_proto::inventory::{ContainerSetContent, ContainerSetSlot};
use acacia_java_proto::item::ItemStack;

use crate::inventory::{HOTBAR_START, Inventory, MAIN_START, SLOTS};

/// The player slots at the end of a container menu.
const PLAYER_SLOTS: usize = 36;
const MAIN_ROWS: usize = 27;

#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub id: i32,
    /// `minecraft:menu` registry id.
    pub menu: i32,
    /// Plain-text titles only (see [`OpenScreen::title_text`]).
    pub title: Option<String>,
    /// The container's own slots; empty until the server sends the contents.
    pub slots: Vec<Option<ItemStack>>,
    /// What clicks in this window echo.
    pub state_id: i32,
}

/// Where a menu slot lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum At {
    Container(usize),
    /// A window-0 slot.
    Player(usize),
}

impl Inventory {
    pub(crate) fn open(&mut self, screen: &OpenScreen) {
        self.window = Some(Window { id: screen.container, menu: screen.menu, title: screen.title_text(), slots: Vec::new(), state_id: 0 });
    }

    pub(crate) fn close(&mut self, id: i32) {
        if self.window.as_ref().is_some_and(|w| w.id == id) {
            self.window = None;
        }
    }

    pub(crate) fn fill_window(&mut self, packet: ContainerSetContent) {
        let Some(window) = self.window.as_mut().filter(|w| w.id == packet.container.0) else { return };
        let mut items = packet.items;
        let player = items.split_off(items.len().saturating_sub(PLAYER_SLOTS));
        window.slots = items;
        window.state_id = packet.state_id.0;
        self.cursor = packet.carried;
        let own = window.slots.len();
        for (i, item) in player.into_iter().enumerate() {
            if let Some(At::Player(slot)) = self.locate(own + i) {
                self.set_slot(slot, item);
            }
        }
    }

    pub(crate) fn set_window_slot(&mut self, packet: ContainerSetSlot) {
        let Some(window) = self.window.as_mut().filter(|w| w.id == packet.container.0) else { return };
        window.state_id = packet.state_id.0;
        if let Some(at) = usize::try_from(packet.slot).ok().and_then(|i| self.locate(i)) {
            self.put(at, packet.item);
        }
    }

    /// The open menu's slot count: window 0's, or the container's plus the player's.
    pub fn menu_len(&self) -> usize {
        self.window.as_ref().map_or(SLOTS, |w| w.slots.len() + PLAYER_SLOTS)
    }

    /// Slot `i` of the open menu, in its own numbering.
    pub fn menu_slot(&self, i: usize) -> Option<&ItemStack> {
        match self.locate(i)? {
            At::Container(c) => self.window.as_ref()?.slots.get(c)?.as_ref(),
            At::Player(p) => self.slot(p),
        }
    }

    /// The open menu's id and the state id its clicks echo.
    pub(crate) fn menu_ids(&self) -> (i32, i32) {
        self.window.as_ref().map_or((0, self.state_id()), |w| (w.id, w.state_id))
    }

    pub(crate) fn locate(&self, i: usize) -> Option<At> {
        let Some(window) = &self.window else { return (i < SLOTS).then_some(At::Player(i)) };
        let own = window.slots.len();
        match i.checked_sub(own) {
            None => Some(At::Container(i)),
            Some(p @ 0..MAIN_ROWS) => Some(At::Player(MAIN_START + p)),
            Some(p @ MAIN_ROWS..PLAYER_SLOTS) => Some(At::Player(HOTBAR_START + p - MAIN_ROWS)),
            Some(_) => None,
        }
    }

    pub(crate) fn put(&mut self, at: At, item: Option<ItemStack>) {
        match at {
            At::Container(c) => {
                if let Some(slot) = self.window.as_mut().and_then(|w| w.slots.get_mut(c)) {
                    *slot = item;
                }
            }
            At::Player(p) => self.set_slot(p, item),
        }
    }
}
