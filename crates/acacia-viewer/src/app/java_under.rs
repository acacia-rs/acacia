//! The Java screens the viewer shows without a menu open (sign, book, level loading), and the
//! credits roll closing itself.

use std::time::Instant;

use super::App;
use super::java_menu::view;
use super::menu::Menu;
use crate::ui::java_screens::{self, View};

impl App {
    /// The Java screen shown when no menu is: the sign editor, the book, or `LevelLoadingScreen`
    /// while the player has not arrived.
    pub(super) fn java_under(&self) -> Option<View> {
        if let Some(sign) = &self.shown.sign {
            let line = &sign.lines[sign.line];
            let lines = std::array::from_fn(|i| sign.lines[i].text());
            return Some(View::Sign { lines, line: sign.line, cursor: line.before_caret().len(), hanging: sign.hanging });
        }
        // TODO: `BookSignScreen` is not ported; signing shows acacia_ui::book's prompt.
        if let Some(page) = self.book_page().filter(|p| p.signing.is_none()) {
            let held = self.play.me.as_ref().and_then(|me| me.book.as_ref()).map_or(&[][..], |b| &b.0);
            let pages = self.shown.book.as_ref().and_then(|b| b.written.clone()).unwrap_or_else(|| held.to_vec());
            return Some(View::Book { pages, page: page.page, writing: page.writing, caret: page.caret });
        }
        (self.player.is_none() && self.mode == super::Mode::Play).then_some(View::Loading)
    }

    /// `WinScreen.tick`: the credits roll, once past its end, goes back as Esc does.
    pub(super) fn close_finished_credits(&mut self, now: Instant) {
        let Some(menu @ Menu::Options) = self.menu else { return };
        let View::Win { ticks } = view(&self.java, menu, &self.settings, now) else { return };
        let (size, scale) = self.gui();
        if self.ui.java_credits_finished(ticks, java_screens::gui_size(size, scale)) {
            self.menu_back();
        }
    }
}
