//! The open screen's input in the Java look: presses, releases, drags, the wheel and the slot keys
//! go to the Java screens (`ui::java_gui::screens`); the Bedrock look keeps `screen.rs`'s.

use std::time::Instant;

use acacia_ui::input::Key as EditKey;
use acacia_ui::java::container::input::{Button, Key};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

use super::App;
use crate::ui::{JavaEvent, JavaScreen};

impl App {
    /// `event` to the Java screen; false in the Bedrock look.
    fn java_event(&mut self, event: JavaEvent) -> bool {
        let (size, scale) = self.gui();
        let screen = JavaScreen { entries: &self.shown.items, creative: self.shown.creative.map(|c| c.0), name: &self.shown.name };
        let at = (&self.inventory, screen, self.play.me.as_ref());
        let Some(out) = self.ui.java_screen_event(self.settings.look, at, (size, scale, self.mouse), &mut self.shown.java, event) else { return false };
        for command in out.commands {
            let _ = self.net.commands.send(command);
        }
        if let Some(tab) = out.tab {
            self.open_tab(tab);
        }
        if out.close {
            self.close_inventory();
        }
        true
    }

    /// A button on the open screen; false when the Bedrock look's handling is wanted.
    pub(super) fn java_button(&mut self, button: MouseButton, pressed: bool) -> bool {
        let button = match button {
            MouseButton::Left => Button::Left,
            MouseButton::Right => Button::Right,
            MouseButton::Middle => Button::Middle,
            _ => return false,
        };
        let event = if pressed {
            let double = self.shown.java.press(button, Instant::now());
            JavaEvent::Press { button, shift: self.shift, double }
        } else {
            JavaEvent::Release { button, shift: self.shift }
        };
        self.java_event(event)
    }

    pub(super) fn java_moved(&mut self) {
        if self.screen_open {
            self.java_event(JavaEvent::Move);
        }
    }

    pub(super) fn java_scroll(&mut self, lines: f32) -> bool {
        self.java_event(JavaEvent::Scroll(f64::from(lines)))
    }

    /// A key on the open screen: the recipe book's search box when it is typed in, else the slot
    /// keys over the hovered slot (1-9 swap with the hotbar, F with the offhand, Q drops one, all
    /// with Control).
    /// True when the key was taken.
    pub(super) fn java_key(&mut self, code: KeyCode) -> bool {
        if self.shown.java.book_typing() {
            let edit = match code {
                KeyCode::Backspace => EditKey::Backspace,
                KeyCode::Delete => EditKey::Delete,
                KeyCode::ArrowLeft => EditKey::Left,
                KeyCode::ArrowRight => EditKey::Right,
                KeyCode::Home => EditKey::Home,
                KeyCode::End => EditKey::End,
                // Typed as text, so it does not close the screen.
                KeyCode::KeyE => return true,
                _ => return false,
            };
            let book = &mut self.shown.java.book;
            book.search.key(edit);
            book.page = 0;
            return true;
        }
        if self.names() {
            return false;
        }
        let key = match code {
            _ if DIGITS.contains(&code) => Key::Hotbar(DIGITS.iter().position(|&d| d == code).unwrap_or(0) as i32),
            KeyCode::KeyF => Key::SwapOffhand,
            KeyCode::KeyQ => Key::Drop { all: self.java.ctrl },
            _ => return false,
        };
        self.java_event(JavaEvent::Key(key))
    }

    /// Text typed into the recipe book's search box.
    pub(super) fn java_text(&mut self, text: &str) {
        let book = &mut self.shown.java.book;
        book.search.insert(text);
        book.page = 0;
    }
}

const DIGITS: [KeyCode; 9] =
    [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9];
