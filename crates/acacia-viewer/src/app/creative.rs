//! The creative inventory's tab, its scrolling and clicks, and the wheel on any open screen's list.

use acacia_bot::items::Click;
use acacia_ui::inventory::{self, CREATIVE_GRID as GRID, Layout};

use super::App;
use super::screen::slot_ref;
use crate::control::{Command, CreativeEntry, Stack};

impl App {
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

    /// The wheel on an open screen: the creative items or a pick list move a row per notch.
    pub(super) fn screen_scroll(&mut self, lines: f32) {
        let step = |row: usize, rows: usize| (row as f32 - lines.signum()).clamp(0.0, rows as f32) as usize;
        let rows = self.creative_rows();
        if let Some((_, row)) = &mut self.shown.creative {
            return *row = step(*row, rows);
        }
        let Some(list) = self.layout().picks() else { return };
        let total = self.inventory.picks.len().max(self.inventory.patterns.len()).max(self.inventory.trade.as_ref().map_or(0, |t| t.offers.len()));
        self.shown.scroll = step(self.shown.scroll, total.div_ceil(list.columns).saturating_sub(list.rows));
    }

    /// Index of the first pick the open screen's list shows.
    pub(super) fn first_pick(&self) -> usize {
        self.layout().picks().map_or(0, |list| self.shown.scroll * list.columns)
    }

    /// A click on the creative screen: a tab, an item (one, or a stack with Shift), or the hotbar.
    pub(super) fn creative_click(&mut self, click: Click, at: [f32; 2]) {
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
}
