//! The inventory screen (E) and container screens (a chest clicked): opening frees the mouse,
//! clicks go to the slot under it.

use acacia_bot::items::{Click, SlotRef, ui};
use acacia_bot::proto::types::GameMode;
use acacia_ui::inventory::{self, Bench, CREATIVE_GRID as GRID, Layout, Slot};
use acacia_ui::input::Key;
use acacia_ui::recipes;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

use super::App;
use crate::control::{Command, CreativeEntry, Inventory, Stack};

/// An anvil takes names this long.
const NAME_MAX: usize = 30;

/// What the open screen shows beyond its slots.
pub(super) struct Shown {
    /// What is picked from a list: a stonecutter's result, a beacon's two powers.
    pub pick: [Option<usize>; 2],
    /// The text in an open anvil's name box.
    pub name: acacia_ui::widget::TextEdit,
    /// The creative inventory's items, and its open tab and scrolled rows while it shows.
    pub items: Vec<CreativeEntry>,
    pub creative: Option<(usize, usize)>,
}

impl Default for Shown {
    fn default() -> Self {
        Shown { pick: [None; 2], name: acacia_ui::widget::TextEdit::new("", NAME_MAX), items: Vec::new(), creative: None }
    }
}

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

    /// E: the player's own screen, or in creative mode the creative inventory.
    pub(super) fn open_inventory(&mut self) {
        let creative = self.play.me.as_ref().is_some_and(|m| m.game_mode == GameMode::Creative) && !self.shown.items.is_empty();
        self.shown.creative = creative.then_some((0, 0));
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
            self.shown.pick = [None; 2];
        }
        if inventory.bench != self.inventory.bench {
            self.shown.pick = [None; 2];
            self.rename(|name| *name = acacia_ui::widget::TextEdit::new("", NAME_MAX));
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
        if self.shown.creative.is_some() && self.inventory.container.is_none() && self.inventory.bench.is_none() {
            return Layout::Creative;
        }
        match (self.inventory.station, &self.inventory.container) {
            (Some(station), Some(_)) => Layout::Station(station),
            (_, Some(c)) => Layout::Rows((c.slots.len() / 9) as u8),
            _ => self.inventory.bench.map_or(Layout::Player, Layout::Bench),
        }
    }

    fn close_inventory(&mut self) {
        self.screen_open = false;
        self.shown.creative = None;
        let _ = self.net.commands.send(Command::Inventory(false));
        self.grab(true);
    }

    /// Whether the open screen has a name box (an anvil's), which typed text goes into.
    pub(super) fn names(&self) -> bool {
        self.screen_open && matches!(self.layout(), Layout::Bench(bench) if bench.name_box().is_some())
    }

    /// Edits the name box and tells the bot loop, which works out the renamed result.
    fn rename(&mut self, edit: impl FnOnce(&mut acacia_ui::widget::TextEdit)) {
        let before = self.shown.name.text();
        edit(&mut self.shown.name);
        let now = self.shown.name.text();
        if now != before {
            let _ = self.net.commands.send(Command::Name((!now.is_empty()).then_some(now)));
        }
    }

    pub(super) fn screen_text(&mut self, text: &str) {
        self.rename(|name| name.insert(text));
    }

    /// Keys while the screen is open: E or Esc close it; a name box takes E as a letter.
    pub(super) fn screen_key(&mut self, code: KeyCode) {
        let naming = self.names();
        let edit = match code {
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Delete => Key::Delete,
            KeyCode::ArrowLeft => Key::Left,
            KeyCode::ArrowRight => Key::Right,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
            KeyCode::Escape => return self.close_inventory(),
            KeyCode::KeyE if !naming => return self.close_inventory(),
            _ => return,
        };
        if naming {
            self.rename(|name| _ = name.key(edit));
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
        if layout == Layout::Creative {
            return self.creative_click(click, at);
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
            self.shown.pick[0] = Some(i);
            return;
        }
        if layout == Layout::Bench(Bench::Beacon) {
            if let Some(i) = inventory::hit_pick(layout, size, crate::stations::BEACON.len(), at) {
                return self.beacon_click(i);
            }
        }
        let picked = self.shown.pick[0].and_then(|i| self.inventory.picks.get(i));
        let command = match inventory::hit(layout, size, at) {
            Some(Slot::Result) if !self.inventory.picks.is_empty() => match picked {
                Some(&(id, _)) => Command::TakeCut { id, all: click == Click::Shift },
                None => return,
            },
            Some(Slot::Result) => {
                let name = Some(self.shown.name.text()).filter(|n| !n.is_empty() && self.names());
                Command::TakeCrafted { all: click == Click::Shift, name }
            }
            // A beacon takes one item: the held stack is not put down whole.
            Some(Slot::Ui(ui::BEACON_PAYMENT)) if self.inventory.cursor.is_some() => Command::Click(SlotRef::Ui(ui::BEACON_PAYMENT), Click::Right),
            Some(slot) => Command::Click(slot_ref(slot), click),
            None if !inventory::inside(layout, size, at) => Command::DropCursor { one: click == Click::Right },
            None => return,
        };
        let _ = self.net.commands.send(command);
    }

    /// The open creative tab's entries, in order.
    fn creative_tab(&self) -> impl Iterator<Item = &CreativeEntry> {
        let tab = self.shown.creative.map_or(0, |(tab, _)| tab);
        self.shown.items.iter().filter(move |e| e.tab == tab)
    }

    /// Rows the open tab can be scrolled by.
    fn creative_rows(&self) -> usize {
        self.creative_tab().count().div_ceil(GRID[0]).saturating_sub(GRID[1])
    }

    /// The items the creative screen shows now, its tab, and how far it is scrolled (0 to 1).
    pub(super) fn creative_view(&self) -> Option<(Vec<Stack>, usize, f32)> {
        let (tab, row) = self.shown.creative.filter(|_| self.layout() == Layout::Creative)?;
        let shown = self.creative_tab().skip(row * GRID[0]).take(GRID[0] * GRID[1]).map(|e| e.stack.clone()).collect();
        Some((shown, tab, row as f32 / self.creative_rows().max(1) as f32))
    }

    /// The wheel on the creative screen: a row per notch.
    pub(super) fn screen_scroll(&mut self, lines: f32) {
        let rows = self.creative_rows();
        if let Some((_, row)) = &mut self.shown.creative {
            *row = (*row as f32 - lines.signum()).clamp(0.0, rows as f32) as usize;
        }
    }

    /// A click on the creative screen: a tab, an item (one, or a stack with Shift), or the hotbar.
    fn creative_click(&mut self, click: Click, at: [f32; 2]) {
        let (layout, size) = (Layout::Creative, self.gui_size());
        let Some((tab, row)) = self.shown.creative else { return };
        if let Some(picked) = inventory::hit_tab(layout, size, at) {
            self.shown.creative = Some((picked, 0));
        } else if let Some(i) = inventory::hit_pick(layout, size, GRID[0] * GRID[1], at) {
            let entry = self.shown.items.iter().filter(|e| e.tab == tab).nth(row * GRID[0] + i);
            if let Some(entry) = entry {
                let _ = self.net.commands.send(Command::Creative { id: entry.id, count: if click == Click::Shift { 64 } else { 1 } });
            }
        } else if let Some(slot) = inventory::hit(layout, size, at) {
            let _ = self.net.commands.send(Command::Click(slot_ref(slot), click));
        } else if !inventory::inside(layout, size, at) {
            let _ = self.net.commands.send(Command::DropCursor { one: click == Click::Right });
        }
    }

    /// A beacon button: a power, one of the two second powers (again to drop it), or the confirm.
    fn beacon_click(&mut self, button: usize) {
        match button {
            0..=4 => self.shown.pick[0] = Some(button),
            crate::stations::BEACON_DONE => {
                if let Some((primary, secondary)) = crate::stations::beacon_powers(self.shown.pick) {
                    let _ = self.net.commands.send(Command::Beacon(primary, secondary));
                }
            }
            _ => self.shown.pick[1] = Some(button).filter(|b| self.shown.pick[1] != Some(*b)),
        }
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
