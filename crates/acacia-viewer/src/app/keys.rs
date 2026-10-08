//! Keys, buttons and the wheel, by mode. F6 switches between playing and the free camera.

use winit::event::MouseButton;
use winit::keyboard::KeyCode;

use super::{App, Mode};
use crate::control::Command;

impl App {
    pub(super) fn key(&mut self, code: KeyCode, pressed: bool) {
        if self.ui.chat.is_open() {
            if pressed {
                self.chat_key(code);
            }
            return;
        }
        match (code, pressed) {
            (KeyCode::KeyT | KeyCode::Slash, true) if self.mode == Mode::Play => {
                self.play.release_all();
                self.ui.chat.open(if code == KeyCode::Slash { "/" } else { "" });
            }
            (KeyCode::Escape, true) => self.grab(false),
            (KeyCode::F6, true) => self.switch_mode(),
            (KeyCode::F3, true) => self.show_debug = !self.show_debug,
            (KeyCode::F5, true) if self.mode == Mode::Play => self.play.next_perspective(),
            (KeyCode::KeyF, true) if self.mode == Mode::Fly => {
                if let Some(p) = self.player {
                    self.camera.position = p;
                }
            }
            (KeyCode::KeyC, true) if self.mode == Mode::Fly => self.settings.change_and_save(|s| s.cave_culling = !s.cave_culling),
            (KeyCode::KeyL, true) => {
                self.settings.change_and_save(|s| s.look = s.look.next());
                if let Some(world) = self.renderer.as_ref().and_then(|r| r.world().cloned()) {
                    self.show_world(world);
                }
            }
            (KeyCode::KeyV, true) if self.mode == Mode::Fly => {
                self.settings.change_and_save(|s| s.vsync = !s.vsync);
                if let Some(r) = &mut self.renderer {
                    r.set_vsync(self.settings.vsync);
                }
            }
            _ => match self.mode {
                Mode::Fly => self.input.key(code, pressed),
                Mode::Play => self.play.key(code, pressed),
            },
        }
    }

    fn chat_key(&mut self, code: KeyCode) {
        let chat = &mut self.ui.chat;
        match code {
            KeyCode::Escape => chat.close(),
            KeyCode::Enter | KeyCode::NumpadEnter => {
                if let Some(text) = chat.submit() {
                    let _ = self.net.commands.send(Command::Chat(text));
                }
            }
            KeyCode::Backspace => chat.backspace(),
            KeyCode::ArrowUp => chat.recall(true),
            KeyCode::ArrowDown => chat.recall(false),
            _ => {}
        }
    }

    /// Text typed while the chat was already open (the key that opens it types nothing).
    pub(super) fn text(&mut self, text: &str, chat_was_open: bool) {
        if chat_was_open {
            self.ui.chat.type_text(text);
        }
    }

    fn switch_mode(&mut self) {
        self.input.release_all();
        self.play.release_all();
        self.mode = match self.mode {
            Mode::Play => Mode::Fly,
            Mode::Fly => Mode::Play,
        };
    }

    pub(super) fn button(&mut self, button: MouseButton, pressed: bool) {
        if !self.grabbed {
            if pressed && button == MouseButton::Left {
                self.grab(true);
            }
            return;
        }
        if self.mode == Mode::Play {
            self.play.button(button, pressed);
        }
    }

    pub(super) fn scroll(&mut self, lines: f32) {
        match self.mode {
            Mode::Fly => self.input.scroll(lines),
            Mode::Play => self.play.scroll(lines),
        }
    }

    pub(super) fn mouse_motion(&mut self, dx: f64, dy: f64) {
        if !self.grabbed {
            return;
        }
        match self.mode {
            Mode::Fly => self.input.mouse(&mut self.camera, dx, dy),
            Mode::Play => self.play.mouse(&mut self.camera, dx, dy),
        }
    }
}
