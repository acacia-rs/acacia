//! Server forms: shown while the bot has one open, answered with the mouse and keyboard. Input
//! reaches a form after the pause menu and before the inventory screen and chat.

use acacia_bot::forms::Form;
use acacia_ui::input::{Input, Key, Mods};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

use super::App;
use crate::control::Command;
use crate::forms::{self, FormScreen};

/// `ACACIA_FORM=<file>`: a form shown from the start, for unattended screenshots. Never answered,
/// and the server's forms don't replace it.
const SHOT_FORM_ID: u32 = u32::MAX;

impl App {
    /// The form the bot reports open; the same one again changes nothing.
    pub(super) fn show_form(&mut self, form: Option<Form>) {
        if self.form.as_ref().is_some_and(|f| f.id >= forms::SIGN_EDITOR_ID) {
            return;
        }
        match form {
            Some(form) if self.form.as_ref().is_some_and(|f| f.id == form.id) => {}
            Some(form) => {
                if self.form.is_none() {
                    self.play.release_all();
                    self.grab(false);
                }
                self.form = Some(FormScreen::new(&form));
            }
            None if self.form.is_some() => self.close_form(),
            None => {}
        }
    }

    /// The sign editor the server opened, on the side's `text` so far; `None` once it is closed.
    pub(super) fn show_sign_editor(&mut self, text: Option<String>) {
        match text {
            Some(text) => {
                if self.form.is_none() {
                    self.play.release_all();
                    self.grab(false);
                }
                self.form = Some(FormScreen::sign_editor(&text));
            }
            None if self.form.as_ref().is_some_and(|f| f.id == forms::SIGN_EDITOR_ID) => self.close_form(),
            None => {}
        }
    }

    pub(super) fn shot_form() -> Option<FormScreen> {
        let json = std::fs::read_to_string(std::env::var_os("ACACIA_FORM")?).ok()?;
        Form::parse(SHOT_FORM_ID, &json).map(|f| FormScreen::new(&f))
    }

    fn close_form(&mut self) {
        self.form = None;
        if self.menu.is_none() && !self.screen_open {
            self.grab(true);
        }
    }

    /// Lays the form out for this frame's look and size, with the mouse where it is.
    pub(super) fn update_form(&mut self) {
        let look = self.settings.look;
        let ([w, h], scale) = self.gui();
        let size = [(w / scale) as f32, (h / scale) as f32];
        let mouse = self.gui_mouse();
        let Some(form) = &mut self.form else { return };
        let theme = self.ui.theme_mut(look);
        form.update(look, theme, size, &mut self.form_images);
        form.handle(&Input::Move(mouse), theme);
    }

    fn form_input(&mut self, input: Input) {
        let look = self.settings.look;
        let Some(form) = &mut self.form else { return };
        let Some(outcome) = form.handle(&input, self.ui.theme_mut(look)) else { return };
        let _ = match form.id {
            SHOT_FORM_ID => Ok(()),
            forms::SIGN_EDITOR_ID => self.net.commands.send(Command::WriteSign(forms::sign_text(outcome))),
            id => self.net.commands.send(Command::AnswerForm(id, forms::reply(outcome))),
        };
        self.close_form();
    }

    pub(super) fn form_key(&mut self, code: KeyCode, pressed: bool) {
        let key = match code {
            KeyCode::Tab => Key::Tab,
            KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
            KeyCode::Space => Key::Space,
            KeyCode::Escape => Key::Escape,
            KeyCode::ArrowLeft => Key::Left,
            KeyCode::ArrowRight => Key::Right,
            KeyCode::ArrowUp => Key::Up,
            KeyCode::ArrowDown => Key::Down,
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Delete => Key::Delete,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
            _ => return,
        };
        if pressed {
            self.form_input(Input::Key(key, Mods { shift: self.shift, ctrl: false }));
        }
    }

    pub(super) fn form_text(&mut self, text: &str) {
        self.form_input(Input::Text(text.to_owned()));
    }

    pub(super) fn form_button(&mut self, button: MouseButton, pressed: bool) {
        if button != MouseButton::Left {
            return;
        }
        let at = self.gui_mouse();
        self.form_input(if pressed { Input::Press(at) } else { Input::Release(at) });
    }

    pub(super) fn form_scroll(&mut self, lines: f32) {
        self.form_input(Input::Wheel(lines));
    }
}
