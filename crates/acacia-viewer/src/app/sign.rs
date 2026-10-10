//! The sign editor: opened by the server (a sign placed or clicked), typed into line by line, and
//! closed with Done or Esc, which both write the sign as Java's do.

use acacia_ui::input::Key;
use acacia_ui::sign_editor::{self, LINES};
use acacia_ui::widget::{TextEdit, contains};
use winit::keyboard::KeyCode;

use super::App;
use crate::control::Command;

/// Characters a line may hold; its width on the sign is what limits it.
const LINE_MAX: usize = 100;

pub(super) struct SignEdit {
    pub lines: [TextEdit; LINES],
    pub line: usize,
}

impl App {
    /// The sign editor the server opened, on the side's `text` so far; `None` once it is closed.
    pub(super) fn show_sign_editor(&mut self, text: Option<String>) {
        match text {
            Some(text) => {
                let mut lines = text.split('\n');
                self.shown.sign = Some(SignEdit { lines: std::array::from_fn(|_| TextEdit::new(lines.next().unwrap_or(""), LINE_MAX)), line: 0 });
                self.play.release_all();
                self.grab(false);
            }
            None if self.shown.sign.take().is_some() => self.leave_sign(),
            None => {}
        }
    }

    fn leave_sign(&mut self) {
        if self.menu.is_none() && self.form.is_none() && !self.screen_open {
            self.grab(true);
        }
    }

    fn write_sign(&mut self) {
        let Some(sign) = self.shown.sign.take() else { return };
        let text = sign.lines.iter().map(TextEdit::text).collect::<Vec<_>>().join("\n");
        let _ = self.net.commands.send(Command::WriteSign(Some(text.trim_end_matches('\n').to_owned())));
        self.leave_sign();
    }

    pub(super) fn sign_key(&mut self, code: KeyCode) {
        let Some(sign) = &mut self.shown.sign else { return };
        let edit = match code {
            KeyCode::Escape => return self.write_sign(),
            KeyCode::ArrowUp => return sign.line = (sign.line + LINES - 1) % LINES,
            KeyCode::ArrowDown | KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Tab => return sign.line = (sign.line + 1) % LINES,
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Delete => Key::Delete,
            KeyCode::ArrowLeft => Key::Left,
            KeyCode::ArrowRight => Key::Right,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
            _ => return,
        };
        sign.lines[sign.line].key(edit);
    }

    /// Typed text goes into the line while it still fits the sign.
    pub(super) fn sign_text(&mut self, text: &str) {
        let look = self.settings.look;
        let Some(sign) = &mut self.shown.sign else { return };
        let mut line = sign.lines[sign.line].clone();
        line.insert(text);
        if sign_editor::fits(self.ui.theme(look), &line.text()) {
            sign.lines[sign.line] = line;
        }
    }

    pub(super) fn sign_click(&mut self) {
        if contains(sign_editor::done_rect(self.gui_size()), self.gui_mouse()) {
            self.write_sign();
        }
    }
}
