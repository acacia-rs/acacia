//! The inventory screen (E) and container screens (a chest clicked): opening frees the mouse,
//! clicks go to the slot under it.

use acacia_bot::items::{Click, SlotRef};
use acacia_ui::inventory::{self, Layout, Slot};
use acacia_ui::recipes;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

use super::App;
use crate::control::{Command, Inventory};

impl App {
    /// `ACACIA_USE=secs`: one right-click on the targeted block or entity, that long after the player
    /// arrived (after the setup commands have built what is to be opened).
    pub(super) fn drive_use(&mut self) {
        let Some(delay) = self.auto_use else { return };
        if self.play.me.is_none() {
            return;
        }
        let at = *self.use_at.get_or_insert_with(|| std::time::Instant::now() + std::time::Duration::from_secs_f32(delay));
        if std::time::Instant::now() >= at && self.mode == super::Mode::Play && self.play.aims_at_something() {
            self.auto_use = None;
            tracing::info!(block = ?self.play.target.as_ref().map(|t| t.block), "ACACIA_USE");
            self.play.button(MouseButton::Right, true);
            self.play.button(MouseButton::Right, false);
        }
    }

    pub(super) fn open_inventory(&mut self) {
        self.show_screen();
        let _ = self.net.commands.send(Command::Inventory(true));
    }

    fn show_screen(&mut self) {
        self.play.release_all();
        self.screen_open = true;
        self.grab(false);
    }

    /// New slots from the bot. A container the server opened (a chest clicked) opens the screen;
    /// one it closed closes it.
    pub(super) fn set_inventory(&mut self, inventory: Inventory) {
        let opened = |i: &Inventory| i.container.is_some() || i.bench.is_some() || i.trade.is_some();
        let (had, has) = (opened(&self.inventory), opened(&inventory));
        if inventory.picks != self.inventory.picks {
            self.pick = None;
        }
        self.inventory = inventory;
        if has && !self.screen_open {
            self.show_screen();
        } else if had && !has && self.screen_open {
            self.screen_open = false;
            self.grab(true);
        }
    }

    pub(super) fn layout(&self) -> Layout {
        if self.inventory.trade.is_some() {
            return Layout::Trade;
        }
        match (self.inventory.station, &self.inventory.container) {
            (Some(station), Some(_)) => Layout::Station(station),
            (_, Some(c)) => Layout::Rows((c.slots.len() / 9) as u8),
            _ => self.inventory.bench.map_or(Layout::Player, Layout::Bench),
        }
    }

    fn close_inventory(&mut self) {
        self.screen_open = false;
        let _ = self.net.commands.send(Command::Inventory(false));
        self.grab(true);
    }

    /// Keys while the screen is open: E or Esc close it.
    pub(super) fn screen_key(&mut self, code: KeyCode) {
        if matches!(code, KeyCode::KeyE | KeyCode::Escape) {
            self.close_inventory();
        }
    }

    pub(super) fn screen_click(&mut self, button: MouseButton, shift: bool) {
        let size = self.gui_size();
        let at = self.gui_mouse();
        let click = match (button, shift) {
            (MouseButton::Left, true) => Click::Shift,
            (MouseButton::Left, false) => Click::Left,
            (MouseButton::Right, _) => Click::Right,
            _ => return,
        };
        let layout = self.layout();
        let book = layout.has_book().then(|| recipes::hit(layout, size, self.inventory.craftable.len(), at)).flatten();
        if let Some(i) = book {
            let name = self.inventory.craftable[i].name.clone();
            let _ = self.net.commands.send(Command::Craft { name, table: self.inventory.workbench });
            return;
        }
        let offers = self.inventory.trade.as_ref().map_or(&[][..], |t| &t.offers);
        if let Some(i) = inventory::hit_pick(layout, size, offers.len(), at) {
            if offers[i].open {
                let _ = self.net.commands.send(Command::Trade(i));
            }
            return;
        }
        if let Some(i) = inventory::hit_pick(layout, size, self.inventory.enchants.len(), at) {
            let _ = self.net.commands.send(Command::Enchant(i));
            return;
        }
        if let Some(i) = inventory::hit_pick(layout, size, self.inventory.picks.len(), at) {
            self.pick = Some(i);
            return;
        }
        let picked = self.pick.and_then(|i| self.inventory.picks.get(i));
        let command = match inventory::hit(layout, size, at) {
            Some(Slot::Result) if !self.inventory.picks.is_empty() => match picked {
                Some(&(id, _)) => Command::TakeCut { id, all: click == Click::Shift },
                None => return,
            },
            Some(Slot::Result) => Command::TakeCrafted { all: click == Click::Shift },
            Some(slot) => Command::Click(slot_ref(slot), click),
            None if !inventory::inside(layout, size, at) => Command::DropCursor { one: click == Click::Right },
            None => return,
        };
        let _ = self.net.commands.send(command);
    }

    /// The window's size in GUI pixels and the scale.
    pub(super) fn gui(&self) -> ([u32; 2], u32) {
        let size = self.window.as_ref().map_or([1, 1], |w| w.inner_size().into());
        (size, acacia_ui::scale::gui_scale(size[0], size[1], self.settings.gui_scale))
    }

    fn gui_size(&self) -> [f32; 2] {
        let ([w, h], scale) = self.gui();
        [(w / scale) as f32, (h / scale) as f32]
    }

    pub(super) fn gui_mouse(&self) -> [f32; 2] {
        let (_, scale) = self.gui();
        [(self.mouse[0] / f64::from(scale)) as f32, (self.mouse[1] / f64::from(scale)) as f32]
    }
}

/// The bot's name for a screen slot.
fn slot_ref(slot: Slot) -> SlotRef {
    match slot {
        Slot::Main(i) => SlotRef::Main(i),
        Slot::Armor(i) => SlotRef::Armor(i),
        Slot::Offhand => SlotRef::Offhand,
        Slot::Container(i) => SlotRef::Container(i),
        Slot::Ui(i) => SlotRef::Ui(i),
        Slot::Result => SlotRef::CREATED_OUTPUT,
    }
}
