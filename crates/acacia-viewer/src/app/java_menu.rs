//! The Java look's screens: the menu state shown as Java's screens (ui/java_gui/menus.rs), their
//! input mapped to the viewer's actions, and the chat input (`ChatScreen`).

use std::time::Instant;

use acacia_ui::java::chat::{ChatAction, ChatScreen};
use acacia_ui::java::screen::confirm::{NO, YES};
use acacia_ui::java::screen::options::input::{Event, event};
use acacia_ui::java::widget::input::{Fired, Interaction, Key};
use winit::keyboard::KeyCode;

use super::App;
use super::menu::Menu;
use crate::control::Command;
use crate::settings::{LookChoice, Settings};
use crate::ui::java_screens::{self, Input, Reply, Shown, View};

/// The Java screens' state between frames.
#[derive(Default)]
pub struct JavaScreens {
    pub interaction: Interaction,
    /// When the player died (the death screen's buttons wait 20 ticks), and when the title
    /// confirmation opened (so do its).
    died: Option<Instant>,
    confirm: Option<Instant>,
    /// The options pages open, the innermost last, and when the innermost opened (the credits
    /// roll scrolls from then).
    pub pages: Vec<&'static str>,
    page_opened: Option<Instant>,
    /// Why the session ended.
    pub reason: String,
    /// The chat input, and when it opened.
    chat: Option<(ChatScreen, Instant)>,
    /// The left button is held (motion drags), and where the mouse was, in window pixels.
    held: bool,
    last_mouse: [f64; 2],
    pub ctrl: bool,
}

impl JavaScreens {
    /// `menu` opened: no focus, the options from their first page.
    pub fn opened(&mut self, menu: Menu) {
        self.interaction = Interaction::default();
        match menu {
            Menu::Death => self.died = Some(Instant::now()),
            Menu::TitleConfirm => self.confirm = Some(Instant::now()),
            Menu::Options => self.pages = vec!["options"],
            Menu::Pause | Menu::Disconnected => {}
        }
    }
}

/// Game ticks from `since` to `now`.
fn ticks(since: Option<Instant>, now: Instant) -> u32 {
    since.map_or(0, |s| (now.saturating_duration_since(s).as_millis() / 50) as u32)
}

/// The screen `menu` shows.
pub fn view(java: &JavaScreens, menu: Menu, settings: &Settings, now: Instant) -> View {
    match menu {
        Menu::Pause => View::Pause,
        Menu::Options => match java.pages.last().copied().unwrap_or("options") {
            "win" => View::Win { ticks: java.page_opened.map_or(0.0, |s| now.saturating_duration_since(s).as_secs_f32() * 20.0) },
            page => View::Options { page, settings: settings.clone() },
        },
        Menu::Death => View::Death { ticks: ticks(java.died, now) },
        Menu::TitleConfirm => View::TitleConfirm { ticks: ticks(java.confirm, now) },
        Menu::Disconnected => View::Disconnected { reason: java.reason.clone() },
    }
}

/// What the Java look shows this frame: the chat input completing `players`' names, and the menu,
/// or else `LevelLoadingScreen` while the player has not `arrived`.
pub fn shown<'a>(java: &'a mut JavaScreens, menu: Option<Menu>, arrived: bool, settings: &Settings, players: &[String], now: Instant) -> Shown<'a> {
    let menu = menu.map(|m| (view(java, m, settings, now), java.interaction)).or_else(|| (!arrived).then(|| (View::Loading, Interaction::default())));
    let chat = java.chat.as_mut().map(|(chat, at)| {
        chat.names = players.to_vec();
        (chat, now.saturating_duration_since(*at).as_millis() as u64)
    });
    Shown { chat, menu }
}

/// The GUI's name for a key.
pub fn gui_key(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::ArrowLeft => Key::Left,
        KeyCode::ArrowRight => Key::Right,
        KeyCode::ArrowUp => Key::Up,
        KeyCode::ArrowDown => Key::Down,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Tab => Key::Tab,
        KeyCode::Escape => Key::Escape,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Space => Key::Space,
        KeyCode::KeyA => Key::A,
        _ => return None,
    })
}

impl App {
    pub(super) fn java_look(&self) -> bool {
        self.settings.look == LookChoice::Java
    }

    /// The mouse in GUI pixels, as Java's screens take it.
    fn java_mouse(&self) -> (f64, f64) {
        let (_, scale) = self.gui();
        (self.mouse[0] / f64::from(scale), self.mouse[1] / f64::from(scale))
    }

    pub(super) fn java_menu_key(&mut self, code: KeyCode) {
        if let Some(key) = gui_key(code) {
            self.java_menu_input(Input::Key { key, shift: self.shift });
        }
    }

    /// The left button in a menu: a click, or the end of a drag.
    pub(super) fn java_menu_button(&mut self, pressed: bool) {
        self.java.held = pressed;
        let (x, y) = self.java_mouse();
        self.java_menu_input(if pressed { Input::Click { x, y, shift: self.shift } } else { Input::Release });
    }

    pub(super) fn java_menu_scroll(&mut self, lines: f32) {
        let (x, y) = self.java_mouse();
        self.java_menu_input(Input::Scroll { x, y, amount: f64::from(lines) });
    }

    /// The cursor moved to `to` (window pixels): a drag while the button is held in a menu.
    pub(super) fn cursor_moved(&mut self, to: [f64; 2]) {
        let dy = to[1] - self.java.last_mouse[1];
        self.mouse = to;
        self.java.last_mouse = to;
        if self.java.held && self.menu.is_some() && self.java_look() {
            let (_, scale) = self.gui();
            let (x, y) = self.java_mouse();
            self.java_menu_input(Input::Drag { x, y, dy: dy / f64::from(scale) });
        }
    }

    fn java_menu_input(&mut self, input: Input) {
        let Some(menu) = self.menu else { return };
        let view = view(&self.java, menu, &self.settings, Instant::now());
        let (size, scale) = self.gui();
        let reply = self.ui.java_menu_input(&view, &mut self.java.interaction, java_screens::gui_size(size, scale), input);
        match reply {
            Some(Reply::Close) => self.menu_back(),
            Some(Reply::Fired(fired)) => self.java_fired(menu, &fired),
            None => {}
        }
    }

    /// A button of `menu` pressed, or an option changed.
    fn java_fired(&mut self, menu: Menu, fired: &Fired) {
        match (menu, &*fired.id) {
            (Menu::Pause, "menu.returnToGame") => self.menu_back(),
            (Menu::Pause, "menu.options") => self.open_menu(Menu::Options),
            (Menu::Pause, "menu.disconnect" | "menu.returnToMenu") | (Menu::TitleConfirm, YES) | (Menu::Disconnected, "gui.toTitle") => self.quit = true,
            (Menu::Options, _) => self.options_fired(fired),
            (Menu::Death, "deathScreen.respawn" | "deathScreen.spectate") => self.respawn(),
            (Menu::Death, "deathScreen.titleScreen") => self.open_menu(Menu::TitleConfirm),
            // Back to the death screen (not a new one) until the server has the player alive.
            (Menu::TitleConfirm, NO) => {
                self.respawn();
                self.menu = Some(Menu::Death);
                self.java.interaction = Interaction::default();
            }
            _ => {}
        }
    }

    fn options_fired(&mut self, fired: &Fired) {
        let ([w, h], _) = self.gui();
        let max_scale = acacia_ui::scale::gui_scale(w, h, 0) as i32;
        let values = |id: &str| java_screens::option_value(&self.settings, id);
        match event(&values, max_scale, fired) {
            Event::Open(page) => {
                self.java.pages.push(page);
                self.java.page_opened = Some(Instant::now());
                self.java.interaction = Interaction::default();
            }
            Event::Done => self.menu_back(),
            Event::Set(id, v) => {
                let mut next = self.settings.clone();
                if java_screens::apply_option(&mut next, &id, &v) && next != self.settings {
                    self.settings.change_and_save(|s| *s = next);
                    if id == "enableVsync" {
                        self.apply_vsync();
                    }
                }
            }
            Event::Pressed(_) => {}
        }
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

    /// `ChatScreen` opened holding `initial`; the mouse is let go, as for any screen.
    pub(super) fn open_java_chat(&mut self, initial: &str) {
        let ([w, h], scale) = self.gui();
        let [w, h] = java_screens::gui_size([w, h], scale);
        let history = self.ui.chat.history().len();
        let chat = ChatScreen::new(self.ui.java_font(), initial, history, self.players.clone(), w, h);
        self.java.chat = Some((chat, Instant::now()));
        self.ui.chat.open(initial);
        self.grab(false);
    }

    fn close_java_chat(&mut self) {
        self.java.chat = None;
        self.ui.chat.close();
        self.grab(true);
    }

    /// `ChatScreen.keyPressed`; Enter sends (`handleChatInput`) and closes.
    pub(super) fn java_chat_key(&mut self, code: KeyCode) {
        let Some(key) = gui_key(code) else { return };
        let history = self.ui.chat.history();
        let (shift, ctrl) = (self.shift, self.java.ctrl);
        let Some((chat, _)) = &mut self.java.chat else { return self.close_java_chat() };
        match chat.key(self.ui.java_font(), &history, key, shift, ctrl) {
            ChatAction::None => {}
            ChatAction::Close => self.close_java_chat(),
            ChatAction::Send(text) => {
                self.close_java_chat();
                if !text.is_empty() {
                    self.ui.chat.record(&text);
                    let _ = self.net.commands.send(Command::Chat(text));
                }
            }
        }
    }

    pub(super) fn java_chat_text(&mut self, text: &str) {
        if let Some((chat, _)) = &mut self.java.chat {
            text.chars().for_each(|c| chat.char_typed(self.ui.java_font(), c));
        }
    }

    pub(super) fn java_chat_click(&mut self) {
        let (x, y) = self.java_mouse();
        if let Some((chat, _)) = &mut self.java.chat {
            chat.click(self.ui.java_font(), x, y);
        }
    }

    /// The chat input fitted to a resized window, keeping what was typed.
    pub(super) fn fit_java_chat(&mut self) {
        let ([w, h], scale) = self.gui();
        let [w, h] = java_screens::gui_size([w, h], scale);
        let history = self.ui.chat.history().len();
        if let Some((chat, _)) = &mut self.java.chat
            && (chat.width, chat.height) != (w, h)
        {
            *chat = ChatScreen::new(self.ui.java_font(), &chat.input.value(), history, chat.names.clone(), w, h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_death_screen_counts_ticks_from_the_death_and_the_confirm_from_its_own_opening() {
        let mut java = JavaScreens::default();
        java.opened(Menu::Death);
        let later = Instant::now() + std::time::Duration::from_millis(1500);
        java.opened(Menu::TitleConfirm);
        assert_eq!(view(&java, Menu::Death, &Settings::default(), later), View::Death { ticks: 30 });
        assert!(matches!(view(&java, Menu::TitleConfirm, &Settings::default(), later), View::TitleConfirm { ticks: 29..=30 }));
    }

    #[test]
    fn options_open_on_their_first_page_and_keys_map() {
        let mut java = JavaScreens::default();
        java.opened(Menu::Options);
        java.pages.push("video");
        let settings = Settings::default();
        assert_eq!(view(&java, Menu::Options, &settings, Instant::now()), View::Options { page: "video", settings: settings.clone() });
        java.opened(Menu::Options);
        assert_eq!(java.pages, ["options"]);
        java.pages.push("win");
        java.page_opened = Some(Instant::now());
        let later = Instant::now() + std::time::Duration::from_millis(1000);
        assert!(matches!(view(&java, Menu::Options, &settings, later), View::Win { ticks } if (19.9..=20.1).contains(&ticks)));
        assert_eq!((gui_key(KeyCode::NumpadEnter), gui_key(KeyCode::KeyQ)), (Some(Key::Enter), None));
    }
}
