//! The held book: a right-click opens its pages. A book and quill is typed into at the end of the
//! open page; what changed is sent when it closes. Signing is not done.

use acacia_bot::books::{MAX_PAGE_CHARS, MAX_PAGES, PageEdit};
use acacia_ui::book::{self, Hit, Page};
use winit::keyboard::KeyCode;

use super::super::App;
use crate::control::Command;

/// The open book: the page shown, and a book and quill's pages as typed so far.
pub(in crate::app) struct Open {
    pub page: usize,
    pub written: Option<Vec<String>>,
}

impl Open {
    /// The page shown, of the typed pages or else the held book's.
    pub(in crate::app) fn shown<'a>(&'a self, held: &'a [String]) -> Page<'a> {
        let pages = self.written.as_deref().unwrap_or(held);
        let at = self.page.min(pages.len().saturating_sub(1));
        Page { text: pages.get(at).map_or("", String::as_str), page: at, pages: pages.len().max(1), writing: self.written.is_some() }
    }
}

impl App {
    /// Whether a right-click now opens the held book.
    pub(in crate::app) fn holds_book(&self) -> bool {
        self.play.me.as_ref().is_some_and(|me| me.book.is_some())
    }

    pub(in crate::app) fn open_book(&mut self) {
        let Some((pages, writable)) = self.play.me.as_ref().and_then(|me| me.book.clone()) else { return };
        let written = writable.then(|| if pages.is_empty() { vec![String::new()] } else { pages });
        self.shown.book = Some(Open { page: 0, written });
        self.play.release_all();
        self.grab(false);
        let _ = self.net.commands.send(Command::OpenBook);
    }

    fn book_page(&self) -> Option<Page<'_>> {
        Some(self.shown.book.as_ref()?.shown(&self.play.me.as_ref()?.book.as_ref()?.0))
    }

    /// Closes the book; a book and quill's changed pages go to the server.
    fn close_book(&mut self) {
        let Some(open) = self.shown.book.take() else { return };
        let held = self.play.me.as_ref().and_then(|me| me.book.as_ref()).map_or(&[][..], |b| &b.0);
        for (page, text) in open.written.iter().flatten().enumerate().filter(|(i, text)| held.get(*i) != Some(*text)) {
            let _ = self.net.commands.send(Command::EditBook(PageEdit::Replace { page: page as u8, text: text.clone() }));
        }
        if self.menu.is_none() && self.form.is_none() {
            self.grab(true);
        }
    }

    /// Turns the page; forward from a book and quill's last page adds one.
    fn turn_page(&mut self, forward: bool) {
        let Some(Page { page, pages, .. }) = self.book_page() else { return self.close_book() };
        let Some(open) = &mut self.shown.book else { return };
        open.page = if forward { page + 1 } else { page.saturating_sub(1) };
        match &mut open.written {
            Some(written) if open.page >= written.len() && written.len() < usize::from(MAX_PAGES) => written.push(String::new()),
            _ => open.page = open.page.min(pages - 1),
        }
    }

    pub(in crate::app) fn book_key(&mut self, code: KeyCode) {
        let writing = self.shown.book.as_ref().is_some_and(|b| b.written.is_some());
        match code {
            KeyCode::Escape => self.close_book(),
            KeyCode::KeyE if !writing => self.close_book(),
            KeyCode::ArrowRight | KeyCode::PageDown => self.turn_page(true),
            KeyCode::ArrowLeft | KeyCode::PageUp => self.turn_page(false),
            KeyCode::Enter | KeyCode::NumpadEnter => self.book_text("\n"),
            KeyCode::Backspace => self.edit_page(|text| _ = text.pop()),
            _ => {}
        }
    }

    /// Typed text goes onto the end of the page while the page still holds it.
    pub(in crate::app) fn book_text(&mut self, typed: &str) {
        let look = self.settings.look;
        let Some(before) = self.book_page().filter(|p| p.writing).map(|p| p.text.to_owned()) else { return };
        let text = before + typed;
        if text.chars().count() <= MAX_PAGE_CHARS && book::fits(self.ui.theme(look), &text) {
            self.edit_page(|page| *page = text);
        }
    }

    fn edit_page(&mut self, edit: impl FnOnce(&mut String)) {
        let Some(Open { page, written: Some(pages) }) = &mut self.shown.book else { return };
        if let Some(text) = pages.get_mut(*page) {
            edit(text);
        }
    }

    pub(in crate::app) fn book_click(&mut self) {
        let Some(page) = self.book_page() else { return self.close_book() };
        match book::hit(page, self.gui_size(), self.gui_mouse()) {
            Some(Hit::Done) => self.close_book(),
            Some(Hit::Forward) => self.turn_page(true),
            Some(Hit::Back) => self.turn_page(false),
            None => {}
        }
    }
}
