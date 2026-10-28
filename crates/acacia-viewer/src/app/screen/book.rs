//! The held book: a right-click opens its pages. A book and quill is typed into at the end of the
//! open page; what changed is sent when it closes, or before it is signed under a typed title.

use acacia_bot::books::{MAX_PAGE_CHARS, MAX_PAGES, MAX_TITLE_CHARS, PageEdit};
use acacia_ui::book::{self, Hit, Page};
use winit::keyboard::KeyCode;

use super::super::App;
use crate::control::Command;

/// The open book: the page shown, a book and quill's pages as typed so far, and the title while
/// it is being signed.
pub(in crate::app) struct Open {
    pub page: usize,
    pub written: Option<Vec<String>>,
    pub title: Option<String>,
    /// Bytes of the page before the caret; past its end, the caret is at the end.
    pub caret: usize,
}

/// Bytes of the character before the caret, and of the one after it.
fn around(text: &str, caret: usize) -> [usize; 2] {
    [text[..caret].chars().next_back(), text[caret..].chars().next()].map(|c| c.map_or(0, char::len_utf8))
}

#[cfg(test)]
#[test]
fn the_caret_steps_over_whole_characters() {
    assert_eq!((around("aé", 3), around("aé", 1), around("", 0)), ([2, 0], [1, 2], [0, 0]));
}

impl Open {
    /// The page shown, of the typed pages or else the held book's.
    pub(in crate::app) fn shown<'a>(&'a self, held: &'a [String]) -> Page<'a> {
        let pages = self.written.as_deref().unwrap_or(held);
        let at = self.page.min(pages.len().saturating_sub(1));
        let text = pages.get(at).map_or("", String::as_str);
        Page { text, page: at, pages: pages.len().max(1), writing: self.written.is_some(), caret: self.caret.min(text.len()), signing: self.title.as_deref() }
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
        self.shown.book = Some(Open { page: 0, written, title: None, caret: usize::MAX });
        self.play.release_all();
        self.grab(false);
        let _ = self.net.commands.send(Command::OpenBook);
    }

    pub(in crate::app) fn book_page(&self) -> Option<Page<'_>> {
        Some(self.shown.book.as_ref()?.shown(&self.play.me.as_ref()?.book.as_ref()?.0))
    }

    /// Closes the book; a book and quill's changed pages go to the server, and with a `title` it
    /// is signed.
    fn close_book(&mut self, title: Option<String>) {
        let Some(open) = self.shown.book.take() else { return };
        let held = self.play.me.as_ref().and_then(|me| me.book.as_ref()).map_or(&[][..], |b| &b.0);
        for (page, text) in open.written.iter().flatten().enumerate().filter(|(i, text)| held.get(*i) != Some(*text)) {
            let _ = self.net.commands.send(Command::EditBook(PageEdit::Replace { page: page as u8, text: text.clone() }));
        }
        if let Some(title) = title {
            let _ = self.net.commands.send(Command::SignBook(title));
        }
        if self.menu.is_none() && self.form.is_none() {
            self.grab(true);
        }
    }

    /// Signs under the typed title, once there is one.
    fn finalize_book(&mut self) {
        let title = self.shown.book.as_ref().and_then(|b| b.title.clone()).filter(|t| !t.trim().is_empty());
        if title.is_some() {
            self.close_book(title);
        }
    }

    fn set_title(&mut self, edit: impl FnOnce(Option<String>) -> Option<String>) {
        if let Some(open) = &mut self.shown.book {
            open.title = edit(open.title.take());
        }
    }

    /// Turns the page; forward from a book and quill's last page adds one.
    fn turn_page(&mut self, forward: bool) {
        let Some(Page { page, pages, .. }) = self.book_page() else { return self.close_book(None) };
        let Some(open) = &mut self.shown.book else { return };
        (open.page, open.caret) = (if forward { page + 1 } else { page.saturating_sub(1) }, usize::MAX);
        match &mut open.written {
            Some(written) if open.page >= written.len() && written.len() < usize::from(MAX_PAGES) => written.push(String::new()),
            _ => open.page = open.page.min(pages - 1),
        }
    }

    pub(in crate::app) fn book_key(&mut self, code: KeyCode) {
        let writing = self.shown.book.as_ref().is_some_and(|b| b.written.is_some());
        let signing = self.shown.book.as_ref().is_some_and(|b| b.title.is_some());
        match code {
            KeyCode::Escape if signing => self.set_title(|_| None),
            KeyCode::Enter | KeyCode::NumpadEnter if signing => self.finalize_book(),
            KeyCode::Backspace if signing => self.set_title(|title| title.map(|mut t| (t.pop(), t).1)),
            _ if signing => {}
            KeyCode::Escape => self.close_book(None),
            KeyCode::KeyE if !writing => self.close_book(None),
            // In a book and quill the arrows move the caret; the page keys turn pages.
            KeyCode::ArrowRight if writing => self.edit_page(|text, caret| caret + around(text, caret)[1]),
            KeyCode::ArrowLeft if writing => self.edit_page(|text, caret| caret - around(text, caret)[0]),
            KeyCode::Home => self.edit_page(|_, _| 0),
            KeyCode::End => self.edit_page(|text, _| text.len()),
            KeyCode::ArrowRight | KeyCode::PageDown => self.turn_page(true),
            KeyCode::ArrowLeft | KeyCode::PageUp => self.turn_page(false),
            KeyCode::Enter | KeyCode::NumpadEnter => self.book_text("\n"),
            KeyCode::Backspace => self.edit_page(|text, caret| {
                let start = caret - around(text, caret)[0];
                text.replace_range(start..caret, "");
                start
            }),
            KeyCode::Delete => self.edit_page(|text, caret| {
                text.replace_range(caret..caret + around(text, caret)[1], "");
                caret
            }),
            _ => {}
        }
    }

    /// Typed text goes in at the caret while the page still holds it.
    pub(in crate::app) fn book_text(&mut self, typed: &str) {
        if self.shown.book.as_ref().is_some_and(|b| b.title.is_some()) {
            return self.set_title(|title| title.map(|t| if (t.clone() + typed).chars().count() <= MAX_TITLE_CHARS { t + typed } else { t }));
        }
        let look = self.settings.look;
        let Some(page) = self.book_page().filter(|p| p.writing) else { return };
        let text = format!("{}{typed}{}", &page.text[..page.caret], &page.text[page.caret..]);
        if text.chars().count() <= MAX_PAGE_CHARS && book::fits(self.ui.theme(look), &text) {
            self.edit_page(|page, caret| {
                *page = text;
                caret + typed.len()
            });
        }
    }

    /// Changes the open page of a book and quill: `edit` gets its text and caret, and returns
    /// where the caret is afterwards.
    fn edit_page(&mut self, edit: impl FnOnce(&mut String, usize) -> usize) {
        let Some(Open { page, written: Some(pages), caret, .. }) = &mut self.shown.book else { return };
        if let Some(text) = pages.get_mut(*page) {
            *caret = edit(text, (*caret).min(text.len()));
        }
    }

    pub(in crate::app) fn book_click(&mut self) {
        let Some(page) = self.book_page() else { return self.close_book(None) };
        match book::hit(page, self.gui_size(), self.gui_mouse()) {
            Some(Hit::Done) => self.close_book(None),
            Some(Hit::Sign) => self.set_title(|_| Some(String::new())),
            Some(Hit::Finalize) => self.finalize_book(),
            Some(Hit::Cancel) => self.set_title(|_| None),
            Some(Hit::Forward) => self.turn_page(true),
            Some(Hit::Back) => self.turn_page(false),
            None => {}
        }
    }
}
